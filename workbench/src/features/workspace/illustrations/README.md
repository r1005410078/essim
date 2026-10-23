# 工作台业务空态插画

2026-10-22 按用户要求制作工程草图，使用内置 `image_gen` 工具生成，未使用CLI／API密钥回退。素材均为原始透明PNG，保留alpha，通过Vite打包为本地静态资源；不依赖生成目录或远程图片服务。

- [energy-testbench-light.png](energy-testbench-light.png)：1619×971，中央欢迎页。电脑、网关和单个机柜，以一根连线组织业务场景。
- [battery-cabinet-light.png](battery-cabinet-light.png)：1254×1254，左侧模拟器空态。单个闭合机柜轮廓与四层分区。
- [point-inspector-light.png](point-inspector-light.png)：1254×1254，右侧属性及问题空态。单张点表、三行定义和简洁选中标记。
- [communication-log-light.png](communication-log-light.png)：1536×1024，底部事件／通信空态。接头、虚线路径与记录纸符号，展示为110×74px。

当前使用轻量工程线稿：细灰轮廓、少量浅蓝／浅绿、大面积留白，去掉写实金属质感、密集排线与厚阴影。中央展示区域仍最多500px宽／320px高，侧栏各160px；底部尺寸不变。此前彩色写实版与黑白版保留作设计迭代参考，生产入口不引用。

本版通过内置imagegen重新生成四张透明PNG，完整提示词见[轻量线稿提示词](light-prompts.md)。以下保留旧版生成过程，便于追溯。

`WorkspaceIllustration`仅提供装饰图，空态文案、按钮及可用状态由业务宿主提供。图片的波形、表格与线路是绘画元素，不是运行数据或接线依据；不展示虚构的在线状态。

## 通信空态生成提示词

内置imagegen独立生成，设置 `transparent_background: true`，没有输入参考图：

```text
Use case: stylized-concept. Asset type: compact empty-state illustration for the event log and communication message pane of ESSim, a light-grey desktop energy-storage protocol engineering workbench. Subject: a small realistic industrial Ethernet/serial gateway, one short curving cable with a clearly visible RJ45 plug, and a single small technical log sheet leaning behind the gateway with just a few abstract horizontal rows; two tiny outlined request/response packet shapes hover between the plug and log sheet. A readable, balanced horizontal cluster with only these three primary objects, designed to remain legible when displayed about 110px wide and 75px tall. Match a mature editorial engineering sketch: fine blue-grey pencil/ink contours, delicate sparse hatching, pale blue and mint washes, only a tiny warm amber indicator accent. Light and airy, no dense shadows, not a glossy 3D icon, not generic cloud art. Genuine transparent background including gaps, no white rectangle or surrounding scenery. The cluster fills most of a 3:2 canvas with modest margin. No text, letters, digits, labels, checkmarks, success badges, logos or watermark. This is decorative communication-inspection artwork and must not imply any real connection or received records.
```

## 黑白版编辑提示词

三张分别引用对应的原始PNG，均设置 `transparent_background: true`，共同编辑提示如下：

```text
Edit this existing ESSim business illustration into a strictly monochrome black-and-white engineering pencil/ink sketch for a restrained light-grey desktop UI. Preserve the original composition, all equipment, cables, packet shapes, relative positions, perspective, framing, and transparent silhouette. Remove ALL color: every drawn pixel must be neutral black, white or grey, with no blue, teal, yellow, warm paper tint, sepia or colored highlights. Replace watercolor color blocks with sparse soft graphite hatching; keep crisp fine contours and slightly lighter, airy midtones so the art supports rather than dominates a technical workbench. Keep key business details readable. Preserve the genuinely transparent background and its alpha, including gaps between objects. Do not add background paper, a rectangle, gradients behind the scene, extra props, words, logos or a watermark. This is the same illustration restyled, not a new scene.
```

## 初版生成提示词

三次独立生成，每次采用以下共同提示词，加上对应的构图提示；均设置 `transparent_background: true`，不引用输入图。

共同提示词：

```text
Use case: stylized-concept. Asset type: original empty-state illustration for ESSim, a light desktop engineering workbench for energy-storage Modbus protocol simulation. Visual style: lively editorial engineering sketch, confident slightly imperfect blue-grey ink contours with a few pencil construction marks, delicate hatching, pale blue and mint watercolor-like color blocks, small warm amber accents. Mature friendly technical drawing, crisp silhouettes readable at UI size, not clip art, not generic cloud SaaS. Isolated on genuine transparent background, no white rectangle, no outer border, no drop shadow slab, no words, no letters, no numerals, no logos, no watermark. Objects must fill most of the canvas with only modest breathing room. No large empty margin.
```

中央联调场景：

```text
Wide 3:2 composition. A cohesive miniature energy-storage protocol test bench: left an EMS workstation with a monitor showing abstract waveforms and a compact register-grid sheet, center a small network gateway with two plug-in cables and a few floating outlined request/response packets following curved dotted paths, right two realistic battery rack cabinets with stacked battery trays, one with an open door and visible modular cells. The cables visibly connect the workstation and battery racks. Include one small handheld test meter leaning near the cabinet and restrained lively hand-drawn signal arcs. Use convincing three-quarter isometric perspective and layered depth while retaining airy sketch character. This is a large hero illustration rendered in the welcome workspace, not a complete app mockup; no UI panels or surrounding page. The technical business story should be immediately clear.
```

左侧设备空态：

```text
Square composition, simple bold subject for a 240px-wide navigation empty state. One tall storage battery cabinet in three-quarter perspective, one front door gently open revealing 5 stacked battery trays, a compact industrial gateway beside its base and an unplugged network cable curving toward the viewer, with two small sketch connection sparks. A pale blue grid-paper sheet rests behind the equipment, no written labels. Strong central silhouette. Match the exact engineering ink-sketch and mint-blue washes style. No computer or entire scene.
```

右侧属性空态：

```text
Square composition, clear subject for a 250px-wide property inspector empty state. A large tilted technical clipboard containing an abstract point-table grid and a few blue/green toggle-like marks, a large magnifying glass over one selected row, a small plug-in communication module and a neat caliper-like adjustment tool in front. Little sketch focus rays around the magnifying glass. The visual story is selecting and inspecting device settings, not analytics dashboards. Strong central silhouette. Match the exact engineering ink-sketch and mint-blue washes style. No written labels.
```
