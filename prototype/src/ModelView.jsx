import { useEffect, useRef, useState } from "react";
import * as T from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
export default function ModelView({ type, model }) {
  const host = useRef();
  const [error, setError] = useState("");
  useEffect(() => {
    let alive = true,
      frame;
    setError("");
    const el = host.current;
    let renderer;
    try {
      renderer = new T.WebGLRenderer({ antialias: true, alpha: true });
    } catch {
      setError("当前环境无法创建 3D 视图");
      return;
    }
    renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    el.append(renderer.domElement);
    const scene = new T.Scene(),
      camera = new T.PerspectiveCamera(35, 1, 0.01, 100);
    camera.position.set(4, 3, 5);
    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.target.set(0, 1, 0);
    scene.add(new T.HemisphereLight(0xffffff, 0x758aa4, 3));
    const light = new T.DirectionalLight(0xffffff, 4);
    light.position.set(3, 5, 4);
    scene.add(light);
    const grid = new T.GridHelper(12, 24, 0xb8c8cb, 0xe3e9e9);
    scene.add(grid);
    const objects = new T.Group();
    scene.add(objects);
    const box = (w, h, d, x, y, z, color) => {
      const m = new T.Mesh(
        new T.BoxGeometry(w, h, d),
        new T.MeshStandardMaterial({ color, roughness: 0.65, metalness: 0.15 }),
      );
      m.position.set(x, y, z);
      objects.add(m);
      return m;
    };
    if (!model) {
      box(1.4, 2.1, 0.85, 0, 1.05, 0, 0xd9e2e1);
      box(1.28, 1.95, 0.04, 0, 1.06, 0.45, 0xf8fbfa);
      box(0.47, 0.27, 0.04, 0.1, 1.6, 0.48, 0x123e3a);
      box(0.32, 0.12, 0.045, 0.1, 1.6, 0.51, 0x54c4a7);
      box(0.045, 0.28, 0.04, 0.48, 1.05, 0.5, 0x647772);
      const rows = type === "BMS" ? 7 : 4;
      for (let i = 0; i < rows; i++)
        box(0.9, 0.045, 0.04, 0, 0.4 + i * 0.13, 0.49, 0x9aaea7);
      box(1.48, 0.1, 0.94, 0, 0.05, 0, 0x536a65);
    } else {
      new GLTFLoader().parse(
        model.buffer,
        "",
        (gltf) => {
          if (!alive) {
            dispose(gltf.scene);
            return;
          }
          const object = gltf.scene;
          const bounds = new T.Box3().setFromObject(object);
          const size = bounds.getSize(new T.Vector3()),
            center = bounds.getCenter(new T.Vector3());
          const scale = 2.3 / Math.max(size.x, size.y, size.z, 0.001);
          object.scale.setScalar(scale);
          object.position.set(
            -center.x * scale,
            -bounds.min.y * scale,
            -center.z * scale,
          );
          objects.add(object);
        },
        () => {
          if (alive)
            setError("模型解析失败，请使用未压缩、资源内嵌的 GLB 文件");
        },
      );
    }
    const resize = () => {
      const { width, height } = el.getBoundingClientRect();
      renderer.setSize(width, height);
      camera.aspect = width / Math.max(height, 1);
      camera.updateProjectionMatrix();
    };
    const observer = new ResizeObserver(resize);
    observer.observe(el);
    resize();
    const render = () => {
      controls.update();
      renderer.render(scene, camera);
      frame = requestAnimationFrame(render);
    };
    render();
    return () => {
      alive = false;
      cancelAnimationFrame(frame);
      observer.disconnect();
      controls.dispose();
      dispose(scene);
      renderer.dispose();
      renderer.domElement.remove();
    };
  }, [type, model]);
  return (
    <div className="model-canvas" ref={host}>
      {error && (
        <div className="model-error" role="alert">
          {error}
        </div>
      )}
    </div>
  );
}
function dispose(object) {
  object.traverse((o) => {
    o.geometry?.dispose();
    const ms = Array.isArray(o.material) ? o.material : [o.material];
    ms.forEach((m) => {
      if (!m) return;
      Object.values(m).forEach((v) => {
        if (v?.isTexture) v.dispose();
      });
      m.dispose();
    });
  });
}
