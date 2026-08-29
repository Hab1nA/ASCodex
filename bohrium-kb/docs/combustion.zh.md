# 燃烧科学

> category: discipline | readTime:  | source: https://play.bohrium.com/api/docs/combustion

复现燃烧研究
燃烧科学是 The Playground 的创始学科。平台提供了火焰速度提取、球形火焰分析、机理选择等专业工具。
核心工具
- pyASURF——一维反应流求解器（平面、柱面、球面）
- Cantera——参考计算（FreeFlame、对冲火焰、零维反应器）
- 70+ 机理——从 H2（8 组分）到大碳氢化合物（127+ 组分）

典型工作流
- 解析论文 → 提取条件（φ、T、P、燃料、机理）
- 验证机理 → 零维点火延迟 vs. Cantera
- 一维参考火焰 → 火焰速度 vs. Cantera FreeFlame
- 目标计算 → 球形火焰、对冲火焰等
- 提取可观测量 → SL、Markstein 长度、熄灭应变率
- 与论文图表对比 → SSIM 评分
