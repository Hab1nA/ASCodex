"""CPU-only signed PmuMA4 baseline for the muon edge challenge."""

from __future__ import annotations

import numpy as np
from scipy.ndimage import convolve, gaussian_filter


DEFAULT_CONFIG = {
    "pixel_mm": 1.0,
    "half_width_mm": 120.0,
    "image_z_mm": 0.0,
    "target_smoothing_mm": 1.75,
    "reference_smoothing_mm": 3.5,
    "normalization_border_mm": 95.0,
    "unsharp_sigma": 3.0,
    "unsharp_amount": 1.5,
    "threshold_sigma": 1.5,
    "saturation": 0.0,
}


def _disk_kernel(radius_pixels: float) -> np.ndarray:
    radius = max(1, int(np.ceil(radius_pixels)))
    yy, xx = np.mgrid[-radius : radius + 1, -radius : radius + 1]
    kernel = (xx * xx + yy * yy <= radius_pixels * radius_pixels).astype(float)
    if kernel.sum() == 0:
        kernel[radius, radius] = 1.0
    return kernel / kernel.sum()


def _smooth(image: np.ndarray, radius_mm: float, pixel_mm: float) -> np.ndarray:
    if radius_mm <= 0:
        return image.copy()
    return convolve(image, _disk_kernel(radius_mm / pixel_mm), mode="nearest")


def _histogram(points: np.ndarray, edges: np.ndarray) -> np.ndarray:
    return np.histogram2d(points[:, 0], points[:, 1], bins=(edges, edges))[0].T


def _intersect_two(hits: np.ndarray, z: np.ndarray, image_z: float) -> np.ndarray:
    weight = (image_z - z[0]) / (z[1] - z[0])
    return hits[:, 0] + weight * (hits[:, 1] - hits[:, 0])


def _intersect_fit(hits: np.ndarray, z: np.ndarray, image_z: float) -> np.ndarray:
    centered_z = z - z.mean()
    slope = np.einsum("l,nlc->nc", centered_z, hits) / np.dot(centered_z, centered_z)
    intercept = hits.mean(axis=1) - slope * z.mean()
    return intercept + slope * image_z


def _ratio(
    numerator_points: np.ndarray,
    denominator_points: np.ndarray,
    edges: np.ndarray,
    cfg: dict,
) -> np.ndarray:
    numerator = _histogram(numerator_points, edges) / len(numerator_points)
    denominator = _histogram(denominator_points, edges) / len(denominator_points)
    numerator = _smooth(numerator, cfg["target_smoothing_mm"], cfg["pixel_mm"])
    denominator = _smooth(denominator, cfg["reference_smoothing_mm"], cfg["pixel_mm"])
    threshold = 0.05 / len(denominator_points)
    return np.divide(
        numerator,
        denominator,
        out=np.full_like(numerator, np.nan),
        where=denominator > threshold,
    )


def _robust_scale(image: np.ndarray, border: np.ndarray) -> np.ndarray:
    center = float(np.nanmedian(image[border]))
    mad = float(np.nanmedian(np.abs(image[border] - center)))
    return (image - center) / max(1.4826 * mad, 1e-8)


def reconstruct(hits: np.ndarray, layer_z: np.ndarray, config: dict | None = None) -> np.ndarray:
    """Return a signed 240 x 240 contrast image from four-layer hit positions.

    Positive values indicate locally stronger material interaction than the
    surrounding background; negative values indicate weaker interaction.
    """
    cfg = dict(DEFAULT_CONFIG)
    if config:
        cfg.update(config)
    hits = np.asarray(hits, dtype=np.float64)
    z = np.asarray(layer_z, dtype=np.float64)
    if hits.ndim != 3 or hits.shape[1:] != (4, 2) or z.shape != (4,):
        raise ValueError("expected hits with shape (N, 4, 2) and layer_z with shape (4,)")
    edges = np.arange(
        -cfg["half_width_mm"],
        cfg["half_width_mm"] + cfg["pixel_mm"],
        cfg["pixel_mm"],
    )
    incident = _intersect_two(hits, z, cfg["image_z_mm"])
    transmitted = _intersect_fit(hits, z, cfg["image_z_mm"])
    ratio4 = _ratio(transmitted, incident, edges, cfg)
    centers = 0.5 * (edges[:-1] + edges[1:])
    xx, yy = np.meshgrid(centers, centers)
    border = (np.abs(xx) >= cfg["normalization_border_mm"]) | (
        np.abs(yy) >= cfg["normalization_border_mm"]
    )
    p4 = _robust_scale(-ratio4, border)
    image = p4
    low = gaussian_filter(image, cfg["unsharp_sigma"], mode="nearest")
    image = image + cfg["unsharp_amount"] * (image - low)
    threshold = cfg["threshold_sigma"]
    image = np.sign(image) * np.maximum(np.abs(image) - threshold, 0.0)
    saturation = cfg["saturation"]
    if saturation:
        image = saturation * np.tanh(image / saturation)
    return np.nan_to_num(image, nan=0.0, posinf=0.0, neginf=0.0).astype(np.float32)
