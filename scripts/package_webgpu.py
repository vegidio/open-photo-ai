"""Repackage the official onnxruntime-ep-webgpu wheels into the <dep>_<goos>_<goarch>.7z layout the app installs.

Usage: python package_webgpu.py <version>   (needs py7zr; run from an empty output dir)
Prints the Size and SHA-256 of each archive, i.e. the values internal/artifacts.go pins.

Every shared library the wheel ships beside the plugin is carried along with it. On Windows those are dxil.dll and
dxcompiler.dll, the DirectX shader compiler Dawn's Direct3D 12 backend loads from the plugin's own directory; without
them the plugin registers and lists its adapters but every device request fails ("DynamicLib.Open: dxil.dll"), so
every model falls back to the CPU. The Linux and macOS wheels ship no companions, so their archives are unaffected.
"""
import hashlib, io, json, sys, urllib.request, zipfile, py7zr

version = sys.argv[1] if len(sys.argv) > 1 else "0.3.0"
PLATFORMS = {  # wheel tag -> (goos_goarch, plugin file name inside the wheel)
    "manylinux_2_28_x86_64": ("linux_amd64", "libonnxruntime_providers_webgpu.so"),
    "win_amd64":             ("windows_amd64", "onnxruntime_providers_webgpu.dll"),
    "win_arm64":             ("windows_arm64", "onnxruntime_providers_webgpu.dll"),
    "macosx_14_0_universal2": ("darwin_arm64", "libonnxruntime_providers_webgpu.dylib"),
}
meta = json.load(urllib.request.urlopen(f"https://pypi.org/pypi/onnxruntime-ep-webgpu/{version}/json"))
for u in meta["urls"]:
    tag = next((t for t in PLATFORMS if t in u["filename"]), None)
    if not tag: continue
    platform, lib = PLATFORMS[tag]
    data = urllib.request.urlopen(u["url"]).read()
    assert hashlib.sha256(data).hexdigest() == u["digests"]["sha256"], u["filename"]
    z = zipfile.ZipFile(io.BytesIO(data))
    names = {n.rsplit("/", 1)[-1]: n for n in z.namelist()}
    out = f"webgpu_{platform}.7z"
    with py7zr.SevenZipFile(out, "w") as a:
        a.writestr(z.read(names[lib]), lib)
        for name in sorted(names):
            if name != lib and (name.endswith((".dll", ".so", ".dylib")) or ".so." in name):
                a.writestr(z.read(names[name]), name)
        for extra in ("LICENSE", "LICENSE.txt", "README.md", "ThirdPartyNotices.txt"):
            if extra in names: a.writestr(z.read(names[extra]), extra)
    blob = open(out, "rb").read()
    print(f'"{platform}": {{Hash: "{hashlib.sha256(blob).hexdigest()}", Size: {len(blob)}, Lib: "{lib}"}},  // from {u["filename"]}')
