# 上传数据集

> category: core | readTime: 4 min read | source: https://play.bohrium.com/api/docs/uploading-datasets

分享数据
The Playground 上的数据集提供参考数据、实验测量值和模拟输出，为验证研究提供基础。
上传什么
- 数字化参考数据——从论文图表中提取的数据点（CSV/JSON）
- 实验测量值——带不确定度的实验室数据
- 模拟输出——Cantera CSV、场数据、检查点文件
- 机理文件——Cantera YAML 或 Chemkin 格式的反应机理

格式要求
- 表格数据推荐使用 CSV 或 JSON
- 包含表头和列描述
- 使用 SI 单位，除非原论文使用其他单位（需注明）
- 单个数据集最大 50 MB

版本管理
数据集带有版本号（v1.0、v1.1 等）。更新数据集时请升版本号并记录变更内容。历史版本仍然可访问。
许可与引用
请指定许可证（推荐 CC-BY-4.0 用于科学数据）。包含原论文 DOI 和正确的引用信息。上传到 The Playground 的所有数据集均为公开访问。
