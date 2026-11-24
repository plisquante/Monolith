#!/usr/bin/env python3
"""Generates the Monolith pixel-art logo SVGs (brand/*.svg). Rects only, no fonts, no dependencies."""
import os
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")

FONT = {
 "M": ["10001","11011","10101","10101","10001","10001","10001"],
 "O": ["01110","10001","10001","10001","10001","10001","01110"],
 "N": ["10001","11001","10101","10101","10011","10001","10001"],
 "L": ["10000","10000","10000","10000","10000","10000","11111"],
 "I": ["111","010","010","010","010","010","111"],
 "T": ["11111","00100","00100","00100","00100","00100","00100"],
 "H": ["10001","10001","10001","11111","10001","10001","10001"],
}
S = 3  # pixel scale of the letter font
STONE = ["#b4b4b4", "#a2a2a2", "#939393"]      # face, picked per cell
HI, SH, EDGE, DROP = "#d6d6d6", "#6b6b6b", "#0d0d10", "#000000"

def rect(x, y, w, h, c, o=None):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{c}"' + (f' fill-opacity="{o}"' if o else "") + "/>"

def wordmark_cells(text="MONOLITH"):
    rows, cols, x = 7, 0, 0
    filled = set()
    for i, ch in enumerate(text):
        g = FONT[ch]
        for r, line in enumerate(g):
            for c, v in enumerate(line):
                if v == "1":
                    for dx in range(S):
                        for dy in range(S): filled.add((S * (x + c) + dx, S * r + dy))
        x += len(g[0]) + 1
    return filled, S * (x - 1)

def wordmark_body(u, ox=0, oy=0):
    """3D stone block letters: lit top edges, shaded bottom edges, black outline, hard drop shadow."""
    filled, width = wordmark_cells()
    out = []
    def cell(cx, cy, col, o=None): out.append(rect(ox + (cx + 1) * u, oy + (cy + 1) * u, u, u, col, o))
    outline = {(x + dx, y + dy) for (x, y) in filled for dx in (-1, 0, 1) for dy in (-1, 0, 1)} - filled
    for (x, y) in sorted(outline): cell(x + 1, y + 1, DROP, .35)       # drop shadow
    for (x, y) in sorted(outline): cell(x, y, EDGE)                     # outline
    for (x, y) in sorted(filled):
        col = STONE[(x * 7 + y * 13 + x * y) % 3]
        if (x, y - 1) not in filled: col = HI
        elif (x, y + 1) not in filled: col = SH
        elif (x - 1, y) not in filled and (x + 1, y) in filled: col = STONE[0]
        cell(x, y, col)
    return "".join(out), width + 2, 7 * S + 2

def title_svg():
    u = 7
    body, wc, hc = wordmark_body(u)
    W, H = 1024, 256
    # oyun 1024x256 dokunun yalnızca üst 176 pikselini çizer (4x ölçekte 64 satırın 44'ü): logoyu o bandın içinde tut
    ox, oy = (W - wc * u) // 2, (176 - hc * u) // 2
    body, _, _ = wordmark_body(u, ox, oy)
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" shape-rendering="crispEdges"><title>Monolith</title>{body}</svg>'

# ---------------- simge: 32x32 piksel sahne, gün doğumunda bir monolit ----------------
def icon_cells():
    G = 32
    px = {}
    sky = [(0, "#1a1d3a"), (8, "#252a52"), (14, "#38386b"), (18, "#5b4478"), (21, "#8a5a7a")]
    for y in range(G):
        col = [c for (y0, c) in sky if y >= y0][-1]
        for x in range(G): px[(x, y)] = col
    for (x, y) in [(3,3),(9,6),(25,4),(28,10),(20,2),(6,12),(27,16),(13,9)]: px[(x, y)] = "#ffffff"
    for (x, y) in [(5,8),(23,7),(30,5),(2,16)]: px[(x, y)] = "#8f95c4"
    # güneş: ufukta (y=24) yarım disk, eş merkezli bantlar
    cx, cy = 16, 24
    for y in range(G):
        for x in range(G):
            d = ((x + .5 - cx) ** 2 + (y + .5 - cy) ** 2) ** .5
            if y < 24:
                if d <= 6.5: px[(x, y)] = "#fbe08a"
                elif d <= 9.5: px[(x, y)] = "#f5a742"
                elif d <= 12.5: px[(x, y)] = "#e0663a"
    # zemin
    for y in range(24, G):
        for x in range(G):
            n = (x * 5 + y * 11 + x * y) % 4
            px[(x, y)] = ["#2b2b33", "#25252c", "#303039", "#222229"][n] if y > 24 else "#4a4a56"
    # monolit: x 13..18 (6 geniş), y 6..24 (zeminden yükselir), kenarlar solda aydınlık, sağda koyu
    for y in range(6, 25):
        for x in range(13, 19):
            c = "#0b0b10"
            if x == 13: c = "#2f3040"
            if x == 18: c = "#050507"
            if y == 6: c = "#3f4152"
            px[(x, y)] = c
    px[(13, 6)] = "#575a70"
    for (x, y) in [(14, 9), (14, 10), (14, 17)]: px[(x, y)] = "#171822"   # faint glints
    return px, G

def icon_svg(rx=0):
    px, G = icon_cells()
    u = 512 // G
    runs = []
    for y in range(G):                      # merge horizontal runs of equal colour
        x = 0
        while x < G:
            c = px[(x, y)]; x2 = x
            while x2 + 1 < G and px[(x2 + 1, y)] == c: x2 += 1
            runs.append(rect(x * u, y * u, (x2 - x + 1) * u, u, c)); x = x2 + 1
    clip = f'<clipPath id="r"><rect width="512" height="512" rx="{rx}"/></clipPath>' if rx else ""
    g = f'<g clip-path="url(#r)">{"".join(runs)}</g>' if rx else "".join(runs)
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512" shape-rendering="crispEdges"><title>Monolith</title><defs>{clip}</defs>{g}</svg>'

def lockup_svg():
    """Icon + wordmark, for README and the launcher."""
    px, G = icon_cells()
    body, wc, hc = wordmark_body(8)
    iconsvg = icon_svg()
    inner = iconsvg[iconsvg.index("<defs>"):iconsvg.rindex("</svg>")]
    k = 0.5
    ih = 512 * k
    tw, th = wc * 8, hc * 8
    W = int(ih + 40 + tw); H = int(ih)
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" shape-rendering="crispEdges"><title>Monolith</title>'
            f'<g transform="scale({k})">{inner}</g><g transform="translate({ih + 40:.0f} {(ih - th) / 2:.0f})">{body}</g></svg>')

if __name__ == "__main__":
    for name, svg in (("monolith-icon.svg", icon_svg(0)), ("monolith-icon-round.svg", icon_svg(96)),
                      ("monolith-title.svg", title_svg()), ("monolith-logo.svg", lockup_svg())):
        open(os.path.join(OUT, name), "w", encoding="utf-8").write(svg)
    print("ok")
