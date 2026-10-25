# 上游来源

- 官方仓库：https://github.com/mz-automation/lib60870
- 标签：v2.4.1
- 固定提交：7a388e3e133999e1ca77ba7521d55d074b7cd2bc
- 归档：https://codeload.github.com/mz-automation/lib60870/tar.gz/7a388e3e133999e1ca77ba7521d55d074b7cd2bc
- 下载日期：2026-10-25
- 实际归档 SHA-256：8bde18d06ae131cda2e49f8aa902ba1daaf1b32283afb39d741563e4291e21ff
- 完整源码树 SHA-256：a4ac5fa2cbdd53bfc594d8d5c486ba6d384b1d203e939b36a521e676d9010d7d（按排序相对路径和文件字节递归计算，算法见 src/lib.rs）
- `vendor/lib60870` 是上述完整归档解压内容，保留 101、104、示例、文档及许可，未应用补丁。
- 上游 GPLv3／商业双许可；完整 GPL 原文见 vendor/lib60870/COPYING。包装代码进入本仓库不表示已取得商业授权或完成公开分发准备。

上游该提交的 CMake 版本为 2.4.1，但 `iec60870_common.h` 的版本宏及 `Lib60870_getLibraryVersionInfo()` 仍报告 2.4.0。本项目保持上游源码原样，源码来源与运行时版本分别校验，不自行修改宏。
