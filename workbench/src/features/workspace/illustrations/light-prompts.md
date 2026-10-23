# 轻量工程线稿生成提示词

2026-10-23 使用内置imagegen重新生成四张素材，未使用CLI回退。每张独立生成，没有输入参考图，均设置 `transparent_background: true`。每次实际提示词为共同风格加对应构图。保留当前展示尺寸，减轻轮廓、明暗与细节密度，不通过CSS透明度或灰度滤镜隐藏旧图。

## 共同风格

```text
Use case: stylized-concept. Asset type: sparse, lightweight editorial engineering outline illustration for ESSim, a restrained pale-grey desktop energy-storage protocol workbench. This must be an extremely simple airy technical doodle, NOT a realistic drawing. Visual treatment: a very small number of confident fine single-stroke contours in soft slate grey (#94A2B3), with white/transparent interiors and abundant unmarked space. Use only a few small very pale ice-blue or mint accents, at most 5 percent of the subject. No black outlines, no metallic material, no rendering, no textures, no pencil hatching, no dense repeated strokes, no gradients, no shadows, no cast shadow, no glow, no ground plane, no background blob, no frame or card. Almost everything should remain blank transparent space inside and around the outlines. Every object reduced to its essential recognizable silhouette. No words, numerals, labels, logos, checkmarks or status badges. Genuine transparent alpha background. Object group should fill roughly 80 percent of the frame, not tiny in an enormous margin. Output a polished original raster artwork with the restrained simplicity of a thin-line UI illustration. Keep the palette soft but the contours legible at the stated display size.
```

## energy-testbench-light.png

```text
Wide 3:2 composition for a 500px-wide welcome illustration. Exactly three simple objects in one coherent scene: one desktop monitor on the left, one tiny plain rectangular communication gateway in the middle, and one closed energy-storage battery cabinet on the right. Slight three-quarter view, almost diagrammatic. The monitor has a simple stand and just two sparse abstract lines on its screen. The gateway has two port outlines only. The cabinet has just four large tray divisions and a small handle, no cells, vents, screws, cables inside or open door. One thin gently curved cable links the three objects. Include at most two tiny chevrons on this cable. No desk, keyboard, notebooks, meter, floating screens, extra racks or industrial details. Interior details across the entire scene should be fewer than 25 short strokes. Clear business story: an EMS computer communicating with one storage device.
```

## battery-cabinet-light.png

```text
Square composition for a 160px sidebar illustration. ONE simple closed energy-storage battery cabinet, slightly turned in a modest three-quarter view, with four tray divisions, a tiny plain handle and two short feet. A single small pale-blue accent on one tray edge. Nothing else: no gateway, cable, paper sheet, open door, vents, screws, cells, background or accessories. Fewer than 12 internal strokes. The cabinet outline and blank tray faces should feel calm, light and spacious.
```

## point-inspector-light.png

```text
Square composition for a 160px sidebar and 74px problem-empty-state illustration. ONE slightly tilted technical inspection sheet, with a tiny folded corner, three widely spaced point-table rows, and a single simple circular selection outline over one row. A very small mint accent on that selected row. No clipboard clamp, magnifier handle, caliper, gateway, pencil, extra pages or accessories. Fewer than 15 internal strokes. This one paper silhouette must convey inspecting point definitions, with large transparent blank areas.
```

## communication-log-light.png

```text
Wide 3:2 composition for a 110 by 74px communication-log empty state. A very simple abstract cable linking two small connector outlines, next to ONE tiny log sheet showing only three short horizontal strokes. A few separated dashes imply a request/response path; one dash is very pale blue. No realistic equipment, screws, metallic texture, packet clouds, dashboard, charts, LEDs or elaborate plugs. Fewer than 20 internal strokes. Readable as a tiny quiet engineering doodle, not a detailed scene.
```
