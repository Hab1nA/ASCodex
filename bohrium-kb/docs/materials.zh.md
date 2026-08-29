# 材料科学

> category: discipline | readTime:  | source: https://play.bohrium.com/api/docs/materials

复现材料研究
材料科学复现涉及 DFT 筛选、合金设计、催化和性质预测。
核心工具
- VASP——平面波 DFT
- pymatgen——材料分析和结构操作
- Materials Project API——参考数据

收敛性测试
始终验证以下方面的收敛性：k 点网格、能量截断和超胞尺寸。一个收敛的计算即使与论文不一致，也比一个未收敛但凑巧一致的计算更有价值。
