import bpy
import bmesh
import math
from mathutils import Vector


def clean(obj):
    bm=bmesh.new()
    bm.from_mesh(obj.data)
    bmesh.ops.remove_doubles(bm,verts=list(bm.verts),dist=.0001)
    bmesh.ops.dissolve_degenerate(bm,edges=list(bm.edges),dist=.00001)
    bmesh.ops.recalc_face_normals(bm,faces=bm.faces)
    bm.to_mesh(obj.data)
    bm.free()
    obj.data.update()


def back_normals(obj):
    normals=[None]*len(obj.data.loops)
    for face in obj.data.polygons:
        for li in face.loop_indices:
            v=obj.data.vertices[obj.data.loops[li].vertex_index].co
            x,y,z=v
            qx,qy=max(abs(x)-4,0),max(abs(y)-22,0)
            radial=(qx**2.35+qy**2.35)**(1/2.35)
            horizontal=Vector((math.copysign(qx**1.35,x),math.copysign(qy**1.35,y),0)).normalized()
            n=face.normal.copy()
            for radius,sign in ((16,1),(13.6,-1)):
                if z>=-1:
                    distance=abs(radial-(12+radius))
                    expected=horizontal*sign
                elif radial>=12:
                    distance=abs(math.hypot(radial-12,z+1)-radius)
                    expected=(horizontal*(radial-12)+Vector((0,0,z+1))).normalized()*sign
                else:
                    distance=abs(z+1+radius)
                    expected=Vector((0,0,-sign))
                if distance<.06 and face.normal.dot(expected)>.55:
                    n=expected
                    break
            normals[li]=tuple(n)
    obj.data.normals_split_custom_set(normals)
