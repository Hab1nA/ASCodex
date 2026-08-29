# 数学证明验证

> category: discipline | readTime:  | source: https://play.bohrium.com/api/docs/math

从纸面定理到形式化证明
The Playground 上的数学复现旨在使用 Lean 4 等证明助手将已发表的证明形式化。
工作流
- 阅读定理陈述和证明思路
- 识别关键引理和依赖关系
- 在 Lean 4（或 Coq、Isabelle）中形式化
- 验证所有证明义务

评分
数学复现的评分方式不同：证明要么通过类型检查（匹配），要么不通过（不匹配）。部分评分表示有未完成引理（admitted lemmas）的证明。
