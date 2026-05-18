#!/usr/bin/env python3
"""Generate ELY Browser app icon — source PNG + all platform sizes."""

from PIL import Image, ImageDraw, ImageFilter
import math
import os
import json

RENDER = 3072
FINAL = 1024
GRAD_SMALL = 512  # larger base for smoother gradient

BASE = os.path.dirname(os.path.dirname(__file__))
OUT_DIR = os.path.join(BASE, "generated-icons")
os.makedirs(OUT_DIR, exist_ok=True)


def create_gradient(size):
    img = Image.new("RGB", (size, size))
    for y in range(size):
        for x in range(size):
            nx, ny = x / size, y / size
            r, g, b = 236.0, 224.0, 243.0

            dx = (nx - 0.80) * 1.7
            dy = (ny - 0.14) * 2.1
            d = min(1.0, math.sqrt(dx * dx + dy * dy))
            w = max(0.0, 1.0 - d) ** 1.7
            r += (255 - r) * w; g += (195 - g) * w; b += (212 - b) * w

            dx = (nx - 0.17) * 1.8
            dy = (ny - 0.87) * 1.7
            d = min(1.0, math.sqrt(dx * dx + dy * dy))
            w = max(0.0, 1.0 - d) ** 1.7
            r += (170 - r) * w; g += (190 - g) * w; b += (255 - b) * w

            dx = (nx - 0.22) * 2.0
            dy = (ny - 0.22) * 2.0
            d = min(1.0, math.sqrt(dx * dx + dy * dy))
            w = max(0.0, 1.0 - d) ** 2.2 * 0.35
            r += (255 - r) * w; g += (222 - g) * w; b += (208 - b) * w

            img.putpixel((x, y), (min(255, int(r)), min(255, int(g)), min(255, int(b))))
    return img


def draw_e_rounded(draw, cx, cy, sz):
    """Draw geometric E with slightly rounded bar ends."""
    e_h = int(sz * 0.40)
    e_w = int(sz * 0.30)
    mid_w = int(e_w * 0.70)
    bar = int(sz * 0.064)
    cr = int(bar * 0.28)  # corner radius for exposed ends

    left = cx - e_w // 2
    top = cy - e_h // 2
    right = left + e_w
    bottom = top + e_h
    mid_y = cy
    white = (255, 255, 255, 255)

    # Vertical bar — full height, rounded top-left and bottom-left corners
    draw.rounded_rectangle([left, top, left + bar, bottom], radius=cr, fill=white)
    # Top bar — overlaps vertical bar at left end
    draw.rounded_rectangle([left, top, right, top + bar], radius=cr, fill=white)
    # Middle bar (shorter)
    draw.rounded_rectangle(
        [left, mid_y - bar // 2, left + mid_w, mid_y + bar // 2],
        radius=cr, fill=white,
    )
    # Bottom bar
    draw.rounded_rectangle([left, bottom - bar, right, bottom], radius=cr, fill=white)


def main():
    print("Creating gradient (512x512)...")
    grad_small = create_gradient(GRAD_SMALL)
    grad = grad_small.resize((RENDER, RENDER), Image.BICUBIC)
    # Slight blur to eliminate any banding
    grad = grad.filter(ImageFilter.GaussianBlur(radius=2))

    print("Building icon at 3072...")
    corner_r = int(RENDER * 0.223)
    mask = Image.new("L", (RENDER, RENDER), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [0, 0, RENDER - 1, RENDER - 1], radius=corner_r, fill=255
    )

    icon = Image.new("RGBA", (RENDER, RENDER), (0, 0, 0, 0))
    icon.paste(grad.convert("RGBA"), mask=mask)

    cx, cy = RENDER // 2, RENDER // 2

    # Subtle drop shadow for the E
    shadow = Image.new("RGBA", (RENDER, RENDER), (0, 0, 0, 0))
    draw_e_rounded(ImageDraw.Draw(shadow), cx, cy + int(RENDER * 0.005), RENDER)
    # Convert white shadow pixels to dark translucent
    sd = shadow.load()
    for sy in range(RENDER):
        for sx in range(RENDER):
            _, _, _, a = sd[sx, sy]
            if a > 0:
                sd[sx, sy] = (25, 18, 12, int(a * 0.15))
    shadow = shadow.filter(ImageFilter.GaussianBlur(radius=int(RENDER * 0.01)))
    icon = Image.alpha_composite(icon, shadow)

    # White E glyph
    glyph = Image.new("RGBA", (RENDER, RENDER), (0, 0, 0, 0))
    draw_e_rounded(ImageDraw.Draw(glyph), cx, cy, RENDER)
    icon = Image.alpha_composite(icon, glyph)

    # Downscale to 1024
    source = icon.resize((FINAL, FINAL), Image.LANCZOS)
    src_path = os.path.join(OUT_DIR, "icon-source-1024.png")
    source.save(src_path, "PNG")
    print(f"  Source: {src_path}")

    # ---- Generate all platform sizes from source ----
    print("Generating platform sizes...")

    ios_dir = os.path.join(OUT_DIR, "ios")
    android_dir = os.path.join(OUT_DIR, "android")
    web_dir = os.path.join(OUT_DIR, "web")
    macos_dir = os.path.join(OUT_DIR, "macos")
    for d in [ios_dir, android_dir, web_dir, macos_dir]:
        os.makedirs(d, exist_ok=True)

    # iOS
    ios_sizes = [40, 60, 58, 87, 80, 120, 180, 76, 152, 167, 1024]
    for s in ios_sizes:
        resized = source.resize((s, s), Image.LANCZOS)
        resized.save(os.path.join(ios_dir, f"icon-{s}.png"), "PNG")
    print(f"  iOS: {len(ios_sizes)} sizes")

    # Write Contents.json for Xcode
    contents = {
        "images": [
            {"size": "20x20", "idiom": "iphone", "filename": "icon-40.png", "scale": "2x"},
            {"size": "20x20", "idiom": "iphone", "filename": "icon-60.png", "scale": "3x"},
            {"size": "29x29", "idiom": "iphone", "filename": "icon-58.png", "scale": "2x"},
            {"size": "29x29", "idiom": "iphone", "filename": "icon-87.png", "scale": "3x"},
            {"size": "40x40", "idiom": "iphone", "filename": "icon-80.png", "scale": "2x"},
            {"size": "40x40", "idiom": "iphone", "filename": "icon-120.png", "scale": "3x"},
            {"size": "60x60", "idiom": "iphone", "filename": "icon-120.png", "scale": "2x"},
            {"size": "60x60", "idiom": "iphone", "filename": "icon-180.png", "scale": "3x"},
            {"size": "76x76", "idiom": "ipad", "filename": "icon-76.png", "scale": "1x"},
            {"size": "76x76", "idiom": "ipad", "filename": "icon-152.png", "scale": "2x"},
            {"size": "83.5x83.5", "idiom": "ipad", "filename": "icon-167.png", "scale": "2x"},
            {"size": "1024x1024", "idiom": "ios-marketing", "filename": "icon-1024.png", "scale": "1x"},
        ],
        "info": {"version": 1, "author": "xcode"},
    }
    with open(os.path.join(ios_dir, "Contents.json"), "w") as f:
        json.dump(contents, f, indent=2)

    # Android
    android_sizes = {"mdpi": 48, "hdpi": 72, "xhdpi": 96, "xxhdpi": 144, "xxxhdpi": 192}
    for density, s in android_sizes.items():
        resized = source.resize((s, s), Image.LANCZOS)
        resized.save(os.path.join(android_dir, f"ic_launcher_{density}.png"), "PNG")
    source.resize((512, 512), Image.LANCZOS).save(
        os.path.join(android_dir, "playstore-512.png"), "PNG"
    )
    print(f"  Android: {len(android_sizes) + 1} sizes")

    # Web + PWA
    web_sizes = [16, 32, 48, 72, 96, 128, 144, 152, 192, 384, 512]
    for s in web_sizes:
        resized = source.resize((s, s), Image.LANCZOS)
        resized.save(os.path.join(web_dir, f"icon-{s}.png"), "PNG")
    # Apple touch icon
    source.resize((180, 180), Image.LANCZOS).save(
        os.path.join(web_dir, "apple-touch-icon.png"), "PNG"
    )
    # Favicon .ico (multi-size)
    ico_16 = source.resize((16, 16), Image.LANCZOS)
    ico_32 = source.resize((32, 32), Image.LANCZOS)
    ico_48 = source.resize((48, 48), Image.LANCZOS)
    ico_16.save(os.path.join(web_dir, "favicon.ico"), format="ICO", sizes=[(16, 16), (32, 32), (48, 48)])
    print(f"  Web/PWA: {len(web_sizes) + 2} assets")

    # macOS .iconset (for iconutil → .icns)
    iconset_dir = os.path.join(macos_dir, "AppIcon.iconset")
    os.makedirs(iconset_dir, exist_ok=True)
    macos_sizes = {
        "icon_16x16.png": 16,
        "icon_16x16@2x.png": 32,
        "icon_32x32.png": 32,
        "icon_32x32@2x.png": 64,
        "icon_128x128.png": 128,
        "icon_128x128@2x.png": 256,
        "icon_256x256.png": 256,
        "icon_256x256@2x.png": 512,
        "icon_512x512.png": 512,
        "icon_512x512@2x.png": 1024,
    }
    for name, s in macos_sizes.items():
        resized = source.resize((s, s), Image.LANCZOS)
        resized.save(os.path.join(iconset_dir, name), "PNG")
    print(f"  macOS: {len(macos_sizes)} sizes in AppIcon.iconset")

    print("\nDone! All icons in: generated-icons/")
    print("To build macOS .icns: iconutil -c icns generated-icons/macos/AppIcon.iconset")


if __name__ == "__main__":
    main()
