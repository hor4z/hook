import bpy
import runpy
from pathlib import Path
from mathutils import Vector

HERE=Path(__file__).resolve().parent
scene=bpy.context.scene
back=bpy.data.objects['V2_Back']
if not back.modifiers.get('Weighted surface normals'):
    normal=back.modifiers.new('Weighted surface normals','WEIGHTED_NORMAL')
    normal.keep_sharp=True
    normal.weight=50
orange=bpy.data.materials.new('V2 Tangerine final')
orange.use_nodes=True
bpy.context.view_layer.update()
bsdf=orange.node_tree.nodes.get('Principled BSDF')
bsdf.inputs['Base Color'].default_value=(1.0,.105,.006,1)
bsdf.inputs['Roughness'].default_value=.4
orange.diffuse_color=(1.0,.105,.006,1)
for obj in scene.objects:
    if obj.name in ('V2_Front','V2_Back','V2_Top_button'):
        obj.data.materials.clear()
        obj.data.materials.append(orange)
for obj in scene.objects:
    if obj.type=='LIGHT': obj.data.energy=357500 if obj.location.x==80 else (195000 if obj.location.x==-120 else 312000)
scene.world.node_tree.nodes['Background'].inputs[1].default_value=.4
for obj in scene.objects:
    if obj.type=='MESH' and obj.name.startswith('V2_'):
        for p in obj.data.polygons:
            p.material_index=0
            if obj.name=='V2_Back': p.use_smooth=True
            elif abs(p.normal.z)>.9999: p.use_smooth=False
for mat in bpy.data.materials:
    if mat.use_nodes and mat.node_tree.nodes.get('Principled BSDF'):
        mat.diffuse_color=mat.node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value
scene.render.engine='CYCLES'
scene.cycles.samples=32
scene.cycles.device='CPU'
scene.render.resolution_x=1000
scene.render.resolution_y=1100
scene.render.resolution_percentage=100
cam=scene.camera

def view(name,loc,scale=122):
    cam.location=loc
    cam.rotation_euler=(-Vector(loc)).to_track_quat('-Z','Y' if abs(loc[1])<200 else 'Z').to_euler()
    if name=='front': cam.rotation_euler=(0,0,0)
    if name=='top': cam.rotation_euler=(1.57079632679,0,3.14159265359)
    cam.data.ortho_scale=scale
    bpy.context.view_layer.update()
    scene.render.filepath=str(HERE/'renders'/f'{name}.png')
    bpy.ops.render.render(write_still=True,scene=scene.name)

view('front',(0,0,300))
view('hero',(95,-130,260),125)
view('back',(0,0,-300))
view('rear-three-quarter',(-130,80,-260),125)
view('left',(-300,0,0))
view('top',(0,300,0))
front=bpy.data.objects['V2_Front']
front.location.z=32
for o in scene.objects:
    if o.name.startswith(('V2_Logo','V2_Indicator')): o.location.z+=32
view('exploded',(125,-180,200),160)
front.location.z=0
for o in scene.objects:
    if o.name.startswith(('V2_Logo','V2_Indicator')): o.location.z-=32
cam.location=(95,-130,260)
cam.rotation_euler=(-cam.location).to_track_quat('-Z','Y').to_euler()
cam.data.ortho_scale=125
for screen in bpy.data.screens:
    for area in screen.areas:
        if area.type=='VIEW_3D':
            area.spaces.active.shading.color_type='MATERIAL'
            area.spaces.active.region_3d.view_distance=160
            area.spaces.active.region_3d.view_location=(0,0,0)
            area.spaces.active.region_3d.view_rotation=cam.rotation_euler.to_quaternion()
bpy.ops.wm.save_as_mainfile(filepath=str(HERE/'hook-device-v2.blend'))
