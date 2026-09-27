import math
import os
import sys

import bmesh
import bpy
from mathutils import Vector

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))

W, H, T = 60.0, 90.0, 30.0
ROUND = 11.0
CORNER = 24.0
POWER = 3.0
WALL = 2.0
GAP = 0.2
LIP_WALL = 1.2
LIP_HEIGHT = 2.5
SPLIT = T / 2 - ROUND
PANEL_DEPTH = 0.6
ORANGE = (0.953, 0.357, 0.239)
WHITE = (0.95, 0.95, 0.94)


def outline(w, h, r, n=160):
    pts = []
    for i in range(n):
        a = 2 * math.pi * i / n
        dx, dy = math.cos(a), math.sin(a)
        lo, hi = 0.0, max(w, h)
        for _ in range(40):
            m = (lo + hi) / 2
            x, y = abs(dx * m) - (w / 2 - r), abs(dy * m) - (h / 2 - r)
            qx, qy = max(x, 0.0), max(y, 0.0)
            d = (qx ** POWER + qy ** POWER) ** (1 / POWER) + min(max(x, y), 0.0) - r
            if d < 0:
                lo = m
            else:
                hi = m
        pts.append((dx * lo, dy * lo))
    return pts


def hull(name, points):
    mesh = bpy.data.meshes.new(name)
    bm = bmesh.new()
    verts = [bm.verts.new(p) for p in points]
    bmesh.ops.convex_hull(bm, input=verts)
    loose = [v for v in bm.verts if not v.link_faces]
    bmesh.ops.delete(bm, geom=loose, context="VERTS")
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(mesh)
    bm.free()
    obj = bpy.data.objects.new(name, mesh)
    bpy.context.collection.objects.link(obj)
    return obj


def body(name, inset=0.0, lat=14, lon=28):
    radius = ROUND - inset
    core = outline(W - 2 * ROUND, H - 2 * ROUND, CORNER - ROUND)
    sphere = []
    for i in range(lat + 1):
        th = math.pi * i / lat
        for j in range(lon):
            ph = 2 * math.pi * j / lon
            sphere.append(Vector((math.sin(th) * math.cos(ph), math.sin(th) * math.sin(ph), math.cos(th))) * radius)
    pts = []
    for x, y in core:
        for z in (-(T / 2 - ROUND), T / 2 - ROUND):
            base = Vector((x, y, z))
            pts.extend(base + s for s in sphere)
    return hull(name, pts)


def prism(name, shape, z0, z1):
    return hull(name, [(x, y, z) for x, y in shape for z in (z0, z1)])


def slot(name, length, width, axis, center, depth):
    pts = []
    for i in range(48):
        a = 2 * math.pi * i / 48
        for end in (-1, 1):
            u = end * (length - width) / 2 + math.cos(a) * width / 2
            v = math.sin(a) * width / 2
            for w in (-depth / 2, depth / 2):
                if axis == "z":
                    p = (u, v, w)
                elif axis == "zv":
                    p = (v, u, w)
                elif axis == "x":
                    p = (w, u, v)
                else:
                    p = (u, w, v)
                pts.append(Vector(p) + Vector(center))
    return hull(name, pts)


def cylinder(name, radius, axis, center, depth, n=40):
    return slot(name, 2 * radius, 2 * radius, axis, center, depth)


def boolean(target, cutter, op):
    mod = target.modifiers.new("b", "BOOLEAN")
    mod.operation = op
    mod.solver = "EXACT"
    mod.object = cutter
    bpy.context.view_layer.objects.active = target
    bpy.ops.object.modifier_apply(modifier=mod.name)
    bpy.data.objects.remove(cutter)


def box(name, z0, z1):
    s = 200
    return prism(name, [(-s, -s), (s, -s), (s, s), (-s, s)], z0, z1)


def shell(name, outer, inner):
    a = body(name, outer)
    boolean(a, body(name + "-in", inner), "DIFFERENCE")
    return a


def build():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    front = shell("front", 0.0, WALL)
    boolean(front, box("keep-front", SPLIT, 50), "INTERSECT")
    back = shell("back", 0.0, WALL)
    boolean(back, box("keep-back", -50, SPLIT), "INTERSECT")
    lip = shell("lip", WALL + GAP, WALL + GAP + LIP_WALL)
    boolean(lip, box("keep-lip", SPLIT - 0.4, SPLIT + LIP_HEIGHT), "INTERSECT")
    boolean(back, lip, "UNION")

    panel = outline(W - 18, H - 18, CORNER - 9)
    boolean(front, prism("panel", panel, T / 2 - PANEL_DEPTH, T / 2 + 10), "DIFFERENCE")
    for y in (H / 2 - 5.5, -(H / 2 - 5.5)):
        boolean(front, slot("slot-h", 10.0, 2.2, "z", (0, y, T / 2), 12), "DIFFERENCE")
    for x in (W / 2 - 5.5, -(W / 2 - 5.5)):
        boolean(front, slot("slot-v", 10.0, 2.2, "zv", (x, 16, T / 2), 12), "DIFFERENCE")

    for x in (W / 2, -W / 2):
        boolean(back, slot("side", 10.0, 2.2, "x", (x, 10, -2), 10), "DIFFERENCE")
    holes = [(0.0, 0.0)] + [(3.4 * math.cos(k * math.pi / 3), 3.4 * math.sin(k * math.pi / 3)) for k in range(6)]
    holes += [(6.8 * math.cos(k * math.pi / 6 + math.pi / 12), 6.8 * math.sin(k * math.pi / 6 + math.pi / 12)) for k in range(12)]
    for hx, hy in holes:
        boolean(back, cylinder("grille", 0.8, "z", (hx, 22 + hy, -T / 2), 10), "DIFFERENCE")
    boolean(back, slot("usb-c", 9.2, 3.4, "y", (0, H / 2, -1.5), 12), "DIFFERENCE")
    boolean(back, cylinder("base", 3.0, "y", (0, -H / 2, -1.5), 12), "DIFFERENCE")
    boolean(back, cylinder("base-ring", 4.6, "y", (0, -H / 2 - 5.2, -1.5), 12), "DIFFERENCE")

    for obj in (front, back):
        bpy.context.view_layer.objects.active = obj
        obj.select_set(True)
        bpy.ops.object.shade_auto_smooth(angle=math.radians(35))
        obj.select_set(False)
    return front, back


def export(front, back):
    for obj, file in ((front, "hook-front.stl"), (back, "hook-back.stl")):
        bpy.ops.object.select_all(action="DESELECT")
        obj.select_set(True)
        bpy.context.view_layer.objects.active = obj
        bpy.ops.wm.stl_export(filepath=os.path.join(HERE, file), export_selected_objects=True, apply_modifiers=True)


def material(name, rgb, rough):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    p = m.node_tree.nodes["Principled BSDF"]
    p.inputs["Base Color"].default_value = (*[c ** 2.2 for c in rgb], 1)
    p.inputs["Roughness"].default_value = rough
    return m


def graphics():
    out = []
    before = set(bpy.data.objects)
    bpy.ops.import_curve.svg(filepath=os.path.join(ROOT, "assets", "logo.svg"))
    for obj in set(bpy.data.objects) - before:
        obj.data.dimensions = "2D"
        out.append(obj)
    logo = out[0]
    for o in out[1:]:
        bpy.data.objects.remove(o)
    bpy.context.view_layer.objects.active = logo
    logo.select_set(True)
    bpy.ops.object.origin_set(type="ORIGIN_GEOMETRY", center="BOUNDS")
    size = max(logo.dimensions.x, logo.dimensions.y)
    logo.scale = (22 / size * logo.scale.x,) * 3
    logo.location = (0, 12, T / 2 - PANEL_DEPTH + 0.05)
    logo.rotation_euler = (0, 0, 0)
    dots = []
    for dx in (-5, 0, 5):
        bpy.ops.mesh.primitive_circle_add(vertices=40, radius=1.6, fill_type="NGON", location=(dx, -8, T / 2 - PANEL_DEPTH + 0.05))
        dots.append(bpy.context.active_object)
    return [logo] + dots


def render(front, back, art):
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.samples = 64
    scene.cycles.use_denoising = True
    scene.cycles.device = "CPU"
    scene.render.resolution_x = 900
    scene.render.resolution_y = 900
    scene.render.film_transparent = False
    scene.view_settings.view_transform = "AgX"
    world = bpy.data.worlds.new("world")
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs[0].default_value = (0.92, 0.9, 0.88, 1)
    world.node_tree.nodes["Background"].inputs[1].default_value = 0.9
    scene.world = world
    orange = material("orange", ORANGE, 0.62)
    white = material("print", WHITE, 0.5)
    for obj in (front, back):
        obj.data.materials.clear()
        obj.data.materials.append(orange)
    for a in art:
        a.data.materials.clear()
        a.data.materials.append(white)
    for loc, energy in (((80, 120, 160), 60000), ((-140, -60, 90), 25000), ((0, 40, -160), 20000)):
        light = bpy.data.objects.new("light", bpy.data.lights.new("light", "AREA"))
        light.data.energy = energy
        light.data.size = 120
        light.location = loc
        light.rotation_euler = Vector(loc).to_track_quat("Z", "Y").to_euler()
        scene.collection.objects.link(light)
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    scene.collection.objects.link(cam)
    scene.camera = cam
    views = {
        "frente": ((0, 0, 400), "ORTHO"),
        "espalda": ((0, 0, -400), "ORTHO"),
        "lado-izquierdo": ((-400, 0, 0), "ORTHO"),
        "lado-derecho": ((400, 0, 0), "ORTHO"),
        "arriba": ((0, 400, 0), "ORTHO"),
        "abajo": ((0, -400, 0), "ORTHO"),
        "perspectiva": ((160, -150, 240), "PERSP"),
        "perspectiva-trasera": ((-170, 120, -230), "PERSP"),
    }
    for name, (loc, kind) in views.items():
        cam.location = loc
        up = "Y" if name not in ("arriba", "abajo") else "Z"
        cam.rotation_euler = (Vector((0, 0, 0)) - Vector(loc)).to_track_quat("-Z", up).to_euler()
        cam.data.type = kind
        cam.data.ortho_scale = 110
        cam.data.lens = 70
        cam.data.clip_end = 2000
        scene.render.filepath = os.path.join(HERE, "renders", name + ".png")
        bpy.ops.render.render(write_still=True)
    front.location.z += 28
    for a in art:
        a.location.z += 28
    cam.location = (190, -170, 150)
    cam.rotation_euler = (Vector((0, 0, 10)) - cam.location).to_track_quat("-Z", "Y").to_euler()
    cam.data.type = "PERSP"
    scene.render.filepath = os.path.join(HERE, "renders", "despiece.png")
    bpy.ops.render.render(write_still=True)
    front.location.z -= 28
    for a in art:
        a.location.z -= 28


if __name__ == "__main__":
    args = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    front, back = build()
    export(front, back)
    art = graphics()
    bpy.ops.wm.save_as_mainfile(filepath=os.path.join(HERE, "hook-device.blend"))
    if "--render" in args:
        render(front, back, art)
    for obj in (front, back):
        bm = bmesh.new()
        bm.from_mesh(obj.data)
        closed = all(e.is_manifold for e in bm.edges)
        bm.free()
        d = obj.dimensions
        print(f"PART {obj.name}: {d.x:.1f} x {d.y:.1f} x {d.z:.1f} mm, {len(obj.data.polygons)} faces, closed={closed}")
