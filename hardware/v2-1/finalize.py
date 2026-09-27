import bpy
import importlib.util
from pathlib import Path

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('refinement',HERE/'build.py')
m=importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
m.scene=bpy.context.scene
m.front=bpy.data.objects['V21_Front']
m.back=bpy.data.objects['V21_Back']
m.button=bpy.data.objects['V21_Top_button']
m.art=[o for o in m.scene.objects if o.name.startswith(('V21_Logo','V21_Dot'))]
m.refine.clean(m.back)
m.refine.back_normals(m.back)
m.matte_texture(m.front.data.materials[0])
m.scene.view_settings.view_transform='Standard'
m.scene.view_settings.look='None'
m.scene.view_settings.exposure=0
report=m.validate((m.front,m.back,m.button))
if any(p['nonmanifold_edges'] or p['components']!=1 for p in report):
    raise RuntimeError('Final geometry validation failed')
m.render_all()
bpy.ops.wm.save_as_mainfile(filepath=str(HERE/'hook-device-v2-1.blend'))
