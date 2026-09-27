import bpy
import bmesh
import importlib.util
import json
import math
from pathlib import Path
from mathutils import Vector

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('v1', HERE.parent / 'v1' / 'build.py')
v1 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(v1)
W, H, T = 64.0, 100.0, 34.0
WALL, GAP, SPLIT = 2.4, 0.25, 6.0
N = 192


def outline(w, h, r):
    return v1.outline(w, h, r, N)


def shell(name, outer, inner):
    rings = outer + inner
    verts = [(x, y, z) for w, h, r, z in rings for x, y in outline(w, h, r)]
    faces = []
    for base, count in ((0, len(outer)), (len(outer), len(inner))):
        for k in range(base, base + count - 1):
            for i in range(N):
                j = (i + 1) % N
                faces.append((k*N+i,k*N+j,(k+1)*N+j,(k+1)*N+i))
        faces.append(tuple((base+count-1)*N+i for i in range(N)))
    for i in range(N):
        j = (i+1) % N
        faces.append((i,j,len(outer)*N+j,len(outer)*N+i))
    data = bpy.data.meshes.new(name)
    data.from_pydata(verts, [], faces)
    data.update()
    bm = bmesh.new()
    bm.from_mesh(data)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(data)
    bm.free()
    obj = bpy.data.objects.new(name,data)
    bpy.context.collection.objects.link(obj)
    return obj


def select(obj):
    bpy.ops.object.select_all(action='DESELECT')
    obj.select_set(True)
    bpy.context.view_layer.objects.active=obj


def cut(obj, tool, op='DIFFERENCE'):
    select(obj)
    v1.boolean(obj,tool,op)


def rounded_box(name, dims, loc, radius):
    bpy.ops.mesh.primitive_cube_add(size=1,location=loc)
    obj=bpy.context.object
    obj.name=name
    obj.dimensions=dims
    bpy.ops.object.transform_apply(location=False,rotation=False,scale=True)
    mod=obj.modifiers.new('Edge radius','BEVEL')
    mod.width=radius
    mod.segments=6
    bpy.ops.object.modifier_apply(modifier=mod.name)
    return obj


def mat(name,color,rough=.36):
    return v1.material(name,color,rough)


def build():
    global scene,front,back,button,art,hardware,orange
    scene=bpy.data.scenes.new('HOOK V2 - visual prototype')
    bpy.context.window.scene=scene
    scene.unit_settings.system='METRIC'
    scene.unit_settings.scale_length=.001
    scene.unit_settings.length_unit='MILLIMETERS'
    orange=mat('V2 Orange',(1,.34,.085),.36)
    white=mat('V2 Porcelain',(.97,.97,.95),.42)
    outer=[(64,100,24,6.25),(64,100,24,9),(63.8,99.8,23.9,11),(63,99,23.5,13),(61.5,97.5,22.8,15),(59.4,95.4,21.7,16.5),(57.6,93.6,20.8,17),(56.4,92.4,20.2,16.85),(55.4,91.4,19.7,16.3),(54.4,90.4,19.2,15.3),(53.4,89.4,18.7,14.5),(52,88,18,14.1),(50,86,17,14)]
    inner=[(59.2,95.2,21.6,6.25),(59.2,95.2,21.6,9),(59,95,21.5,11),(58,94,21,12.7),(56.8,92.8,20.4,13.8),(55.8,91.8,19.9,14.1),(54.8,90.8,19.4,13.8),(53.8,89.8,18.9,12.8),(52.8,88.8,18.4,12.1),(51.5,87.5,17.7,11.7),(50,86,17,11.6)]
    front=shell('V2_Front',outer,inner)
    outer=[(64,100,24,6),(64,100,24,0),(63.8,99.8,23.9,-4),(63,99,23.5,-8),(61.4,97.4,22.7,-11),(58.8,94.8,21.4,-13.5),(55.4,91.4,19.7,-15.4),(51.4,87.4,17.7,-16.6),(46,82,15,-17)]
    inner=[(59.2,95.2,21.6,6),(59.2,95.2,21.6,0),(59,95,21.5,-4),(58.2,94.2,21.1,-7.4),(56.6,92.6,20.3,-10),(54.2,90.2,19.1,-12.1),(51,87,17.5,-13.6),(46,82,15,-14.6)]
    back=shell('V2_Back',outer,inner)
    lip=v1.prism('Alignment lip',outline(58.7,94.7,21.35),5.6,8.7)
    cut(lip,v1.prism('Lip inside',outline(56.3,92.3,20.15),5,10))
    cut(back,lip,'UNION')
    for y in (-43,43):
        cut(front,v1.slot('Horizontal recess',12.4,3.2,'z',(0,y,16),10))
    for x in (-26.3,26.3):
        cut(front,v1.slot('Vertical recess',13,3.2,'zv',(x,20,16),10))
    for col in range(-3,4):
        count=7-abs(col)
        for row in range(count):
            cut(back,v1.slot('Grille slot',3.4,1.35,'zv',(col*3.2,23+(row-(count-1)/2)*4.4,-16),9))
    cut(back,v1.slot('USB-C opening',10.0,4.2,'x',(-31,-14,0),12))
    cut(back,v1.cylinder('Status LED',.8,'x',(-31,-23,0),12))
    cut(back,v1.slot('Power switch opening',8,3.2,'x',(31,-29,0),12))
    for obj in (front,back):
        cut(obj,v1.slot('Button clearance',24.6,9.6,'y',(0,49,3),10))
    button=v1.slot('V2_Top_button',24,9,'y',(0,49.5,3),2.4)
    cut(button,v1.slot('Button retaining flange',27,12,'y',(0,47.6,3),1.5),'UNION')
    cut(button,rounded_box('Button actuator',(3.5,4.5,3.5),(0,45.2,3),.4),'UNION')
    for obj in (front,back,button):
        obj.data.materials.append(orange)
        for p in obj.data.polygons: p.use_smooth=True
        select(obj)
        mod=obj.modifiers.new('Small edge highlight','BEVEL')
        mod.width=.16
        mod.segments=3
        mod.limit_method='ANGLE'
        mod.angle_limit=.55
        bpy.ops.object.modifier_apply(modifier=mod.name)
        obj['status']='Visual prototype; fit and hardware retention not validated'
    v1.T=29.2
    v1.PANEL_DEPTH=.6
    art=v1.graphics()
    for obj in art:
        obj.location.z=14.04
        if obj.type=='CURVE':
            obj.name='V2_Logo_print'
            obj.location.y=10.5
        else:
            obj.name='V2_Indicator_print'
            obj.location.x*=1.35
            obj.location.y=-10.5
        obj.data.materials.clear()
        obj.data.materials.append(white)
    hardware=bpy.data.collections.new('V2 - Provisional hardware envelopes (hidden)')
    scene.collection.children.link(hardware)
    green=mat('V2 PCB',(.06,.36,.24),.5)
    gray=mat('V2 Battery',(.63,.67,.72),.5)
    envelopes=[('ESP32', (29,53,10),(-12,-14,5)),('LiPo', (50,34,8),(0,-23,-9.5)),('TP4056',(27,17,5),(-15,-14,-2)),('MT3608',(17,37,7),(16,-24,5)),('MAX98357A',(18,19,5),(16,8,5)),('INMP441',(16,19,4),(-15,27,7)),('Tactile',(6,5,6),(0,42,3)),('SS12D00',(9,9,4),(24,-29,0))]
    for name,size,loc in envelopes:
        obj=rounded_box('Envelope_'+name,size,loc,.4)
        for c in list(obj.users_collection): c.objects.unlink(obj)
        hardware.objects.link(obj)
        obj.data.materials.append(gray if name=='LiPo' else green)
        obj['status']='UNVERIFIED envelope only; not a purchased part specification'
        obj['dimensions_mm']=list(size)
    obj=v1.cylinder('Envelope_Speaker_40x10',20,'z',(0,23,-7),10)
    for c in list(obj.users_collection): c.objects.unlink(obj)
    hardware.objects.link(obj)
    obj.data.materials.append(gray)
    hardware.hide_render=True
    hardware.hide_viewport=True
    scene['status']='VISUAL V2 - not production ready; module sizes provisional'
    scene['envelope_mm']=[W,H,T]
    return front,back,button


def setup():
    scene.render.engine='CYCLES'
    scene.cycles.samples=40
    scene.cycles.use_denoising=True
    scene.render.resolution_x=1000
    scene.render.resolution_y=1100
    scene.render.resolution_percentage=100
    scene.world=bpy.data.worlds.new('V2 Studio')
    scene.world.use_nodes=True
    scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.25,.28,.33,1)
    scene.world.node_tree.nodes['Background'].inputs[1].default_value=.6
    scene.view_settings.view_transform='AgX'
    for loc,energy,size in [((80,100,170),550000,120),((-120,10,90),300000,120),((50,60,-160),480000,100)]:
        data=bpy.data.lights.new('V2 Softbox','AREA')
        data.energy=energy
        data.shape='DISK'
        data.size=size
        obj=bpy.data.objects.new('V2 Softbox',data)
        scene.collection.objects.link(obj)
        obj.location=loc
        obj.rotation_euler=(-obj.location).to_track_quat('-Z','Y').to_euler()
    cam=bpy.data.objects.new('V2 Camera',bpy.data.cameras.new('V2 Camera'))
    scene.collection.objects.link(cam)
    scene.camera=cam
    cam.data.type='ORTHO'
    cam.data.ortho_scale=122
    cam.data.clip_end=2000


def view(name,loc,scale=122):
    cam=scene.camera
    cam.location=loc
    cam.rotation_euler=(-cam.location).to_track_quat('-Z','Y' if abs(loc[1])<200 else 'Z').to_euler()
    cam.data.ortho_scale=scale
    scene.render.filepath=str(HERE/'renders'/f'{name}.png')
    if name=='front': cam.rotation_euler=(0,0,0)
    bpy.context.view_layer.update()
    bpy.ops.render.render(write_still=True,scene=scene.name)


def export(objects):
    report=[]
    for obj in objects:
        select(obj)
        bm=bmesh.new()
        bm.from_mesh(obj.data)
        report.append({'name':obj.name,'nonmanifold_edges':sum(not e.is_manifold for e in bm.edges),'volume_mm3':abs(bm.calc_volume()),'dimensions_mm':list(obj.dimensions)})
        bm.free()
        bpy.ops.wm.stl_export(filepath=str(HERE/(obj.name+'_VISUAL.stl')),export_selected_objects=True,apply_modifiers=True)
    (HERE/'validation.json').write_text(json.dumps(report,indent=2))
    return report


if __name__=='__main__':
    objects=build()
    report=export(objects)
    setup()
    bpy.ops.wm.save_as_mainfile(filepath=str(HERE/'hook-device-v2.blend'))
    import runpy
    runpy.run_path(str(HERE/'finish.py'),run_name='__main__')
    print(json.dumps(report))
