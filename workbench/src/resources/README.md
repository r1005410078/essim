# 内置文档资源

与会话中的采集模型分开管理，项目树“文档资源”与“采集模型”平级。

```text
resources/
  protocols/
    modbus/
      guide.md       # Modbus详细指南，中文Markdown
```

文档正文随客户端打包，可离线读取。分类、标题和稳定资源ID在 `features/resources/catalog.ts` 注册；`ResourceDocument.tsx` 负责Markdown和目录渲染。新增协议资料时按协议建文件夹，再注册文档，不写入模拟器或模型配置。

Dockview以资源ID复用Tab，章节锚点仅负责定位，不产生新Tab。更改章节标题时同步功能码到章节的映射，并运行资源测试。正文说明标准、教学约定及ESSim支持边界；代码或能力变化时同步维护，不能把标准功能介绍当作已实现声明。
