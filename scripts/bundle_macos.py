"""本机开发验证包：构建、复制自行编译库、移除构建路径并 ad-hoc 签名。
不提供公证或公开分发合规承诺。使用：python3 scripts/bundle_macos.py static|dynamic
动态模式另传 --libmodbus-dir <build-shared 输出的绝对前缀>。
"""
import argparse
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
ROOT = Path(__file__).resolve().parents[1]

def run(args, **kwargs):
    return subprocess.run(args, cwd=ROOT, check=True, **kwargs)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('mode', choices=['static', 'dynamic'])
    parser.add_argument('--libmodbus-dir', type=Path)
    args = parser.parse_args()
    if sys.platform != 'darwin':
        parser.error('本入口只验证 macOS；其他平台需要独立验收')
    env = os.environ.copy()
    prefix = args.libmodbus_dir
    if args.mode == 'static':
        if prefix or env.get('LIBMODBUS_DIR') or env.get('LIBMODBUS_PREFIX'):
            parser.error('静态 vendored 模式不能同时设置动态/旧前缀')
    else:
        prefix = prefix or (Path(env['LIBMODBUS_DIR']) if env.get('LIBMODBUS_DIR') else None)
        if prefix is None or not prefix.is_absolute():
            parser.error('动态模式需要自行编译库的绝对前缀')
        env['LIBMODBUS_DIR'] = str(prefix)
    command = ['pnpm','--filter','@essim/workbench','tauri','build','--debug','--bundles','app',
               '--config',json.dumps({'bundle':{'active':True}})]
    if args.mode == 'dynamic':
        command += ['--','--no-default-features']
    run(command, env=env)
    source = ROOT/'target/debug/bundle/macos/ESSim.app'
    destination = ROOT/'target/artifacts'/args.mode/'ESSim.app'
    if destination.exists():
        shutil.rmtree(destination)
    shutil.copytree(source,destination,symlinks=True)
    shutil.copytree(ROOT/'docs/licenses/serialport', destination/'Contents/Resources/third-party/serialport', dirs_exist_ok=True)
    info = plistlib.loads((destination/'Contents/Info.plist').read_bytes())
    binary = destination/'Contents/MacOS'/info['CFBundleExecutable']
    links_modbus = 'libmodbus' in subprocess.check_output(['otool', '-L', str(binary)], text=True)
    license_dir = destination/'Contents/Resources/third-party/libmodbus'
    license_dir.mkdir(parents=True,exist_ok=True)
    package = ROOT/'crates/libmodbus/libmodbus-src'
    shutil.copy2(package/'vendor/libmodbus/COPYING.LESSER',license_dir/'COPYING.LESSER')
    shutil.copy2(package/'SOURCE.md',license_dir/'SOURCE.md')
    if args.mode == 'dynamic' and links_modbus:
        manifest = json.loads((prefix/'libmodbus-build.json').read_text())
        library_name = manifest['library']
        if Path(library_name).name != library_name:
            raise RuntimeError('构建清单包含无效库名')
        frameworks = destination/'Contents/Frameworks'
        frameworks.mkdir(exist_ok=True)
        library = frameworks/library_name
        shutil.copy2(prefix/'lib'/library_name,library)
        run(['install_name_tool','-delete_rpath',str(prefix/'lib'),str(binary)])
        run(['codesign','--force','--sign','-',str(library)])
    run(['codesign','--force','--deep','--sign','-',str(destination)])
    run(['codesign','--verify','--deep','--strict',str(destination)])
    dependencies = subprocess.check_output(['otool','-L',str(binary)],text=True)
    commands = subprocess.check_output(['otool','-l',str(binary)],text=True)
    if '/opt/homebrew' in dependencies or (args.mode=='dynamic' and str(prefix) in commands):
        raise RuntimeError('包仍引用宿主机或构建前缀')
    if links_modbus and ('libmodbus' in dependencies) != (args.mode=='dynamic'):
        raise RuntimeError('最终链接模式与所选模式不符')
    if not links_modbus:
        print('M01配置工作台不链接libmodbus；库的静态／动态模式请用独立测试验证。')
    print(destination)
    print(dependencies)

if __name__ == '__main__':
    main()
