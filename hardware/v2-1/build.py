import bpy
import bmesh
import importlib.util
import json
import math
from pathlib import Path
from mathutils import Vector
from mathutils.bvhtree import BVHTree
from mathutils.geometry import barycentric_transform

SURFACES = {}
STAGES = []


HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('geometry', HERE.parent / 'v1' / 'build.py')
g = importlib.util.module_from_spec(spec)
spec.loader.exec_module(g)
g.POWER = 2.35
refine_spec=importlib.util.spec_from_file_location('refine_surface',HERE/'refine_surface.py')
refine=importlib.util.module_from_spec(refine_spec)
refine_spec.loader.exec_module(refine)
W, H, T, WALL = 64.0, 100.0, 34.0, 2.4
N = 192


def outline(w, h, radius, z, cy=0):
    return [Vector((x, y+cy, z)) for x,y in g.outline(w,h,radius,N)]


def bezier(a,b,c,d,steps):
    return [[(1-t)**3*p+3*(1-t)**2*t*q+3*(1-t)*t*t*r+t**3*s for p,q,r,s in zip(a,b,c,d)] for t in [i/steps for i in range(steps)]]


def taper_rings(ring, center):
    center=Vector(center)
    return [[center+(p-center)*scale for p in ring] for scale in (1,.8,.6,.4,.2,.06)]


def shell(name, outer, inner):
    verts=[tuple(p) for ring in outer+inner for p in ring]
    faces=[]
    for base,count in ((0,len(outer)),(len(outer),len(inner))):
        for k in range(base,base+count-1):
            for i in range(N):
                j=(i+1)%N
                faces.append((k*N+i,k*N+j,(k+1)*N+j,(k+1)*N+i))
        faces.append(tuple((base+count-1)*N+i for i in range(N)))
    for i in range(N):
        j=(i+1)%N
        faces.append((i,j,len(outer)*N+j,len(outer)*N+i))
    mesh=bpy.data.meshes.new(name)
    mesh.from_pydata(verts,[],faces)
    mesh.update()
    bm=bmesh.new()
    bm.from_mesh(mesh)
    bmesh.ops.recalc_face_normals(bm,faces=bm.faces)
    bm.to_mesh(mesh)
    bm.free()
    obj=bpy.data.objects.new(name,mesh)
    bpy.context.collection.objects.link(obj)
    return obj


def select(obj):
    bpy.ops.object.select_all(action='DESELECT')
    obj.select_set(True)
    bpy.context.view_layer.objects.active=obj


def boolean(obj,tool,op='DIFFERENCE'):
    select(obj)
    tool_name=tool.name
    g.boolean(obj,tool,op)
    bm=bmesh.new()
    bm.from_mesh(obj.data)
    STAGES.append({'part':obj.name,'tool':tool_name,'nonmanifold':sum(not e.is_manifold for e in bm.edges)})
    bm.free()


def front_rings(inner=False):
    wall=WALL if inner else 0
    a=outline(64-2*wall,100-2*wall,28-wall,6.25)
    b=outline(64-2*wall,100-2*wall,28-wall,14-wall)
    c=outline(62-2*wall,93-2*wall,27-wall,17-wall,3)
    d=outline(60.6-2*wall,91.4-2*wall,26.3-wall,17-wall,3.5)
    e=outline(58.4-2*wall,89.2-2*wall,25.2-wall,17-wall,3.5)
    f=outline(52,78,22,12.8-wall,5.5)
    floor=outline(48.6,74.6,20.3,12.8-wall,5.5)
    return bezier(a,b,c,d,18)+bezier(d,e,f,floor,22)+taper_rings(floor,(0,5.5,12.8-wall))


def back_rings(inner=False):
    inset=WALL if inner else 0
    radius=16-inset
    rings=[outline(64-2*inset,100-2*inset,28-inset,6)]
    for i in range(33):
        a=math.pi*i/64
        d=inset+radius*(1-math.cos(a))
        rings.append(outline(64-2*d,100-2*d,28-d,-1-radius*math.sin(a)))
    return rings[:-1]+taper_rings(rings[-1],(0,0,-1-radius))


def material(name,color,rough=.46,metallic=0):
    m=bpy.data.materials.new(name)
    m.use_nodes=True
    bpy.context.view_layer.update()
    n=m.node_tree.nodes['Principled BSDF']
    n.inputs['Base Color'].default_value=(*color,1)
    n.inputs['Roughness'].default_value=rough
    n.inputs['Metallic'].default_value=metallic
    n.inputs['Specular IOR Level'].default_value=.3
    m.diffuse_color=(*color,1)
    return m


def matte_texture(mat):
    nodes=mat.node_tree.nodes
    geometry=nodes.new('ShaderNodeNewGeometry')
    noise=nodes.new('ShaderNodeTexNoise')
    noise.inputs['Scale'].default_value=8
    noise.inputs['Detail'].default_value=2
    bump=nodes.new('ShaderNodeBump')
    bump.inputs['Strength'].default_value=.12
    bump.inputs['Distance'].default_value=.025
    mat.node_tree.links.new(geometry.outputs['Position'],noise.inputs['Vector'])
    mat.node_tree.links.new(noise.outputs['Fac'],bump.inputs['Height'])
    mat.node_tree.links.new(bump.outputs['Normal'],nodes['Principled BSDF'].inputs['Normal'])


def shade(obj,mat):
    obj.data.materials.clear()
    obj.data.materials.append(mat)
    for p in obj.data.polygons:
        p.material_index=0
        p.use_smooth=True
    select(obj)
    bevel=obj.modifiers.new('Molded opening radius','BEVEL')
    bevel.width=.2
    bevel.segments=3
    bevel.limit_method='ANGLE'
    bevel.angle_limit=.7
    bpy.ops.object.modifier_apply(modifier=bevel.name)
    refine.clean(obj)
    source=SURFACES.get(obj.name)
    if source:
        source.calc_loop_triangles()
        triangles=[tuple(t.vertices) for t in source.loop_triangles]
        verts=[v.co.copy() for v in source.vertices]
        normals=[v.normal.copy() for v in source.vertices]
        tree=BVHTree.FromPolygons(verts,triangles,all_triangles=True)
        loop_normals=[None]*len(obj.data.loops)
        for face in obj.data.polygons:
            for li in face.loop_indices:
                v=obj.data.vertices[obj.data.loops[li].vertex_index].co
                point,normal,index,distance=tree.find_nearest(v)
                n=face.normal.copy()
                if distance<.075 and face.normal.dot(normal)>.65:
                    ia,ib,ic=triangles[index]
                    n=barycentric_transform(point,verts[ia],verts[ib],verts[ic],normals[ia],normals[ib],normals[ic]).normalized()
                loop_normals[li]=tuple(n)
        obj.data.normals_split_custom_set(loop_normals)
    else:
        normal=obj.modifiers.new('Surface normals','WEIGHTED_NORMAL')
        normal.keep_sharp=True
    if obj.name=='V21_Back': refine.back_normals(obj)


def build():
    global scene,front,back,button,art,orange
    scene=bpy.data.scenes.new('HOOK V2.1 - reference refinement')
    bpy.context.window.scene=scene
    scene.unit_settings.system='METRIC'
    scene.unit_settings.scale_length=.001
    scene.unit_settings.length_unit='MILLIMETERS'
    front=shell('V21_Front',front_rings(),front_rings(True))
    back=shell('V21_Back',back_rings(),back_rings(True))
    SURFACES[front.name]=front.data.copy()
    SURFACES[back.name]=back.data.copy()
    lip=g.prism('Alignment lip',g.outline(58.7,94.7,25.35,N),5.6,8.7)
    boolean(lip,g.prism('Lip cavity',g.outline(56.3,92.3,24.15,N),5,10))
    root=g.prism('Lip root',g.outline(60,96,26,N),4.6,6)
    boolean(root,g.prism('Root cavity',g.outline(56.3,92.3,24.15,N),4,7))
    boolean(lip,root,'UNION')
    boolean(back,lip,'UNION')
    for y in (-33.1,44.9):
        boolean(front,g.slot('Horizontal capsule',12.6,3.25,'z',(0,y,15),14))
    for x in (-25.5,25.5):
        boolean(front,g.slot('Side capsule',13.6,3.35,'zv',(x,18.8,15),14))
    cutters=[]
    for col,count in enumerate((3,4,5,6,5,4,3)):
        x=(col-3)*3.15
        for row in range(count):
            cutters.append(g.slot('Grille tool',3.45,1.45,'zv',(x,20.7+(row-(count-1)/2)*4.7,-15),16))
    bpy.ops.object.select_all(action='DESELECT')
    for obj in cutters: obj.select_set(True)
    bpy.context.view_layer.objects.active=cutters[0]
    bpy.ops.object.join()
    boolean(back,bpy.context.object)
    boolean(back,g.slot('USB-C clearance',10.7,4.8,'x',(-31,-14.2,1),14))
    boolean(back,g.cylinder('Indicator aperture',.75,'x',(-31,-23.7,1),14))
    boolean(back,g.slot('Power switch',7,2.8,'x',(31,-30,-1),14))
    boolean(back,g.slot('Button clearance',25.4,8.9,'y',(0,49,0),7))
    button=g.slot('V21_Top_button',24.8,8.3,'y',(0,49.7,0),1.5)
    boolean(button,g.slot('Retaining flange',26.8,10.3,'y',(0,48.4,0),1.4),'UNION')
    orange=material('V21 Tangerine',(1,.125,.016),.7)
    orange.node_tree.nodes['Principled BSDF'].inputs['Specular IOR Level'].default_value=.22
    matte_texture(orange)
    white=material('V21 White',(.93,.94,.92),.5)
    black=material('V21 USB polymer',(.006,.009,.012),.55)
    steel=material('V21 USB metal',(.38,.4,.43),.27,.8)
    for obj in (front,back,button):
        shade(obj,orange)
        obj['status']='Visual refinement; mechanical retention and fit remain provisional'
    details=bpy.data.collections.new('V21 - Connector appearance only')
    scene.collection.children.link(details)
    socket=g.slot('USB-C metal rim',9.5,3.65,'x',(-31.6,-14.2,1),.6)
    boolean(socket,g.slot('Socket hollow',8.5,2.65,'x',(-31.6,-14.2,1),2))
    dark=g.slot('USB-C dark interior',8.5,2.65,'x',(-31.3,-14.2,1),.45)
    tongue=g.slot('USB-C tongue',6.6,.7,'x',(-31.85,-14.2,1),.4)
    led=g.cylinder('Charge LED lens',.57,'x',(-31.85,-23.7,1),.4)
    for obj,mat in ((socket,steel),(dark,black),(tongue,steel),(led,white)):
        for collection in list(obj.users_collection): collection.objects.unlink(obj)
        details.objects.link(obj)
        obj.data.materials.append(mat)
        obj['status']='Visual detail only; not an exact purchased connector'
    g.T=26.8
    g.PANEL_DEPTH=.6
    art=g.graphics()
    for obj in art:
        obj.location.z=12.84
        if obj.type=='CURVE':
            obj.name='V21_Logo_print'
            obj.location.y=10.6
            obj.scale*=.98
        else:
            obj.name='V21_Dot_print'
            obj.location.x*=1.4
            obj.location.y=-10.8
            obj.scale*=1.16
        obj.data.materials.clear()
        obj.data.materials.append(white)
    select(front)
    scene['status']='Visual reference refinement; not ready for electronics assembly'
    scene['envelope_mm']=[64,100,34]
    return front,back,button


def validate(objects):
    report=[]
    for obj in objects:
        select(obj)
        deps=bpy.context.evaluated_depsgraph_get()
        evaluated=obj.evaluated_get(deps)
        bm=bmesh.new()
        bm.from_mesh(evaluated.to_mesh())
        groups=[]
        unseen=set(bm.verts)
        while unseen:
            stack=[unseen.pop()]
            count=0
            while stack:
                v=stack.pop()
                count+=1
                for e in v.link_edges:
                    n=e.other_vert(v)
                    if n in unseen:
                        unseen.remove(n)
                        stack.append(n)
            groups.append(count)
        report.append({'part':obj.name,'nonmanifold_edges':sum(not e.is_manifold for e in bm.edges),'loose_vertices':sum(not v.link_faces for v in bm.verts),'components':len(groups),'volume_mm3':abs(bm.calc_volume()),'dimensions_mm':list(obj.dimensions)})
        bm.free()
        evaluated.to_mesh_clear()
        bpy.ops.wm.stl_export(filepath=str(HERE/(obj.name+'_VISUAL.stl')),export_selected_objects=True,apply_modifiers=True)
    (HERE/'stages.json').write_text(json.dumps(STAGES,indent=2))
    (HERE/'validation.json').write_text(json.dumps(report,indent=2))
    return report


def studio():
    scene.render.engine='CYCLES'
    scene.cycles.samples=32
    scene.cycles.use_denoising=True
    scene.cycles.device='CPU'
    scene.render.resolution_x=1000
    scene.render.resolution_y=1100
    scene.render.resolution_percentage=100
    scene.world=bpy.data.worlds.new('V21 Studio')
    scene.world.use_nodes=True
    scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.36,.39,.44,1)
    scene.world.node_tree.nodes['Background'].inputs[1].default_value=.35
    scene.view_settings.view_transform='Standard'
    scene.view_settings.look='None'
    for loc,energy,size in [((80,110,150),340000,150),((-120,10,100),140000,150),((60,70,-180),290000,160)]:
        data=bpy.data.lights.new('V21 Softbox','AREA')
        data.energy=energy
        data.shape='DISK'
        data.size=size
        obj=bpy.data.objects.new('V21 Softbox',data)
        scene.collection.objects.link(obj)
        obj.location=loc
        obj.rotation_euler=(-obj.location).to_track_quat('-Z','Y').to_euler()
    cam=bpy.data.objects.new('V21 Camera',bpy.data.cameras.new('V21 Camera'))
    scene.collection.objects.link(cam)
    scene.camera=cam
    cam.data.type='ORTHO'
    cam.data.clip_end=2000


def view(name,loc,scale=120):
    cam=scene.camera
    cam.location=loc
    cam.rotation_euler=(-Vector(loc)).to_track_quat('-Z','Y').to_euler()
    if name=='front': cam.rotation_euler=(0,0,0)
    if name=='left': cam.rotation_euler=(0,-math.pi/2,0)
    if name=='top': cam.rotation_euler=(-math.pi/2,0,0)
    cam.data.ortho_scale=scale
    bpy.context.view_layer.update()
    scene.render.filepath=str(HERE/'renders'/f'{name}.png')
    bpy.ops.render.render(write_still=True,scene=scene.name)


def render_all():
    view('front',(0,0,300))
    view('hero',(100,-130,250),124)
    view('back',(0,0,-300))
    view('rear-three-quarter',(-130,80,-260),124)
    view('left',(-300,0,0))
    view('top',(0,300,0))
    front.location.z=32
    for o in art: o.location.z+=32
    view('exploded',(125,-180,200),160)
    front.location.z=0
    for o in art: o.location.z-=32
    cam=scene.camera
    cam.location=(100,-130,250)
    cam.rotation_euler=(-cam.location).to_track_quat('-Z','Y').to_euler()
    cam.data.ortho_scale=124
    for screen in bpy.data.screens:
        for area in screen.areas:
            if area.type=='VIEW_3D':
                area.spaces.active.shading.color_type='MATERIAL'
                area.spaces.active.region_3d.view_distance=155
                area.spaces.active.region_3d.view_location=(0,0,0)
                area.spaces.active.region_3d.view_rotation=cam.rotation_euler.to_quaternion()


if __name__=='__main__':
    objects=build()
    report=validate(objects)
    studio()
    bpy.ops.wm.save_as_mainfile(filepath=str(HERE/'hook-device-v2-1.blend'))
    render_all()
    bpy.ops.wm.save_as_mainfile(filepath=str(HERE/'hook-device-v2-1.blend'))
    print(json.dumps(report))
