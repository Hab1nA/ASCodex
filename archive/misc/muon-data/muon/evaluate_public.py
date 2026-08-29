#!/usr/bin/env python3
"""Run and score a reconstruction on the public U-shaped target."""

from __future__ import annotations

import argparse
import importlib.util
import json
import time
from pathlib import Path

import numpy as np
from scipy.ndimage import gaussian_filter1d, map_coordinates


IMAGE_SHAPE = (240, 240)
CENTERS = np.arange(-119.5, 120.0, 1.0)
U_VERTICES = np.asarray(
    [
        [-75.0, -75.0],
        [75.0, -75.0],
        [75.0, 75.0],
        [30.0, 75.0],
        [30.0, -30.0],
        [-30.0, -30.0],
        [-30.0, 75.0],
        [-75.0, 75.0],
    ]
)


def _load_solution(path: Path):
    spec = importlib.util.spec_from_file_location("candidate_solution", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    reconstruct = getattr(module, "reconstruct", None)
    if not callable(reconstruct):
        raise TypeError("solution must define callable reconstruct(hits, layer_z, config)")
    return reconstruct


def _sample(image: np.ndarray, x: np.ndarray, y: np.ndarray) -> np.ndarray:
    return map_coordinates(
        image,
        [(y - CENTERS[0]).ravel(), (x - CENTERS[0]).ravel()],
        order=1,
        mode="constant",
        cval=np.nan,
    ).reshape(x.shape)


def _isotonic(values: np.ndarray) -> np.ndarray:
    levels: list[float] = []
    masses: list[float] = []
    starts: list[int] = []
    stops: list[int] = []
    for index, value in enumerate(values):
        levels.append(float(value))
        masses.append(1.0)
        starts.append(index)
        stops.append(index + 1)
        while len(levels) >= 2 and levels[-2] > levels[-1]:
            mass = masses[-2] + masses[-1]
            level = (levels[-2] * masses[-2] + levels[-1] * masses[-1]) / mass
            levels[-2:] = [level]
            masses[-2:] = [mass]
            stops[-2:] = [stops[-1]]
            starts.pop()
    result = np.empty_like(values, dtype=float)
    for level, start, stop in zip(levels, starts, stops):
        result[start:stop] = level
    return result


def _width(distance: np.ndarray, profile: np.ndarray) -> tuple[float, float] | None:
    valid = np.isfinite(profile) & (np.abs(distance) <= 18.0)
    x = distance[valid]
    y = profile[valid]
    if len(y) < 20:
        return None
    y = gaussian_filter1d(y, sigma=2.0, mode="nearest")
    outside = y[(x >= -16.0) & (x <= -10.0)]
    inside = y[(x >= 10.0) & (x <= 16.0)]
    if len(outside) < 2 or len(inside) < 2:
        return None
    low = float(np.median(outside))
    high = float(np.median(inside))
    scale = high - low
    if not np.isfinite(scale) or scale <= 0.0:
        return None
    normalized = _isotonic((y - low) / scale)
    if normalized[0] > 0.2 or normalized[-1] < 0.8:
        return None
    x20 = float(np.interp(0.2, normalized, x))
    x50 = float(np.interp(0.5, normalized, x))
    x80 = float(np.interp(0.8, normalized, x))
    return x80 - x20, x50


def edge_metrics(image: np.ndarray) -> dict[str, float | int | bool | None]:
    distance = np.arange(-20.0, 20.0001, 0.25)
    samples = []
    valid_edges = 0
    for index in range(len(U_VERTICES)):
        start = U_VERTICES[index]
        stop = U_VERTICES[(index + 1) % len(U_VERTICES)]
        vector = stop - start
        length = float(np.linalg.norm(vector))
        tangent = vector / length
        inward = np.asarray([-tangent[1], tangent[0]])
        along = np.arange(10.0, length - 10.0 + 1e-9, 1.0)
        base = start[None, :] + along[:, None] * tangent[None, :]
        locations = base[:, None, :] + distance[None, :, None] * inward[None, None, :]
        sampled = _sample(image, locations[:, :, 0], locations[:, :, 1])
        samples.append(sampled)
        if _width(distance, np.nanmean(sampled, axis=0)) is not None:
            valid_edges += 1
    pooled = _width(distance, np.nanmean(np.concatenate(samples, axis=0), axis=0))
    width = None if pooled is None else float(pooled[0])
    offset = None if pooled is None else float(pooled[1])
    valid = (
        width is not None
        and offset is not None
        and abs(offset) <= 4.0
        and valid_edges == len(U_VERTICES)
    )
    return {
        "valid": valid,
        "knife_edge_width_mm": width,
        "edge_offset_mm": offset,
        "valid_edges": valid_edges,
        "required_edges": len(U_VERTICES),
    }


def main() -> None:
    kit = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser()
    parser.add_argument("solution", type=Path)
    parser.add_argument("--data", type=Path, default=kit / "data" / "public_events.npz")
    parser.add_argument("--image", type=Path, default=Path("public_reconstruction.npy"))
    parser.add_argument("--metrics", type=Path, default=Path("public_metrics.json"))
    args = parser.parse_args()

    reconstruct = _load_solution(args.solution)
    with np.load(args.data, allow_pickle=False) as artifact:
        hits = np.asarray(artifact["hits"], dtype=np.float32)
        layer_z = np.asarray(artifact["layer_z"], dtype=np.float32)
    started = time.monotonic()
    image = np.asarray(
        reconstruct(
            hits,
            layer_z,
            {
                "pixel_mm": 1.0,
                "half_width_mm": 120.0,
                "image_z_mm": 0.0,
                "output_shape": [240, 240],
            },
        )
    )
    runtime = time.monotonic() - started
    if image.shape != IMAGE_SHAPE:
        raise ValueError(f"expected image shape {IMAGE_SHAPE}, got {image.shape}")
    if image.dtype.kind not in "fiu" or not np.isfinite(image).all():
        raise ValueError("reconstruct must return a finite numeric array")
    if float(np.ptp(image)) == 0.0:
        raise ValueError("reconstruct must return a non-constant image")

    metrics = edge_metrics(image.astype(np.float64))
    metrics["runtime_seconds"] = runtime
    args.image.parent.mkdir(parents=True, exist_ok=True)
    args.metrics.parent.mkdir(parents=True, exist_ok=True)
    np.save(args.image, image)
    args.metrics.write_text(json.dumps(metrics, indent=2, allow_nan=False) + "\n")
    print(json.dumps(metrics, indent=2, allow_nan=False))
    if not metrics["valid"]:
        raise SystemExit(2)


if __name__ == "__main__":
    main()
