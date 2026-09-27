import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const IFACE = `<node>
  <interface name="dev.hook.Shell">
    <method name="Windows"><arg type="s" direction="out" name="json"/></method>
    <signal name="Changed"/>
  </interface>
</node>`;

export default class HookExtension extends Extension {
    enable() {
        this._handlers = [];
        this._windows = new Map();
        this._pending = 0;
        this._dbus = Gio.DBusExportedObject.wrapJSObject(IFACE, this);
        this._dbus.export(Gio.DBus.session, '/dev/hook/Shell');
        this._name = Gio.bus_own_name_on_connection(Gio.DBus.session, 'dev.hook.Shell', Gio.BusNameOwnerFlags.REPLACE, null, null);
        const display = global.display;
        this._on(display, 'window-created', (_d, w) => this._track(w));
        this._on(display, 'restacked', () => this._changed());
        this._on(display, 'notify::focus-window', () => this._changed());
        for (const actor of global.get_window_actors())
            this._track(actor.meta_window);
    }

    disable() {
        if (this._pending)
            GLib.source_remove(this._pending);
        this._pending = 0;
        for (const [w, ids] of this._windows)
            ids.forEach(id => w.disconnect(id));
        this._windows.clear();
        for (const [obj, id] of this._handlers)
            obj.disconnect(id);
        this._handlers = [];
        this._dbus?.unexport();
        this._dbus = null;
        if (this._name)
            Gio.bus_unown_name(this._name);
        this._name = 0;
    }

    _on(obj, signal, cb) {
        this._handlers.push([obj, obj.connect(signal, cb)]);
    }

    _track(w) {
        if (!w || this._windows.has(w))
            return;
        const ids = [
            w.connect('position-changed', () => this._changed()),
            w.connect('size-changed', () => this._changed()),
            w.connect('notify::minimized', () => this._changed()),
            w.connect('unmanaged', () => {
                ids.forEach(id => w.disconnect(id));
                this._windows.delete(w);
                this._changed();
            }),
        ];
        this._windows.set(w, ids);
        this._changed();
    }

    _changed() {
        if (this._pending || !this._dbus)
            return;
        this._pending = GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
            this._pending = 0;
            this._dbus?.emit_signal('Changed', null);
            return GLib.SOURCE_REMOVE;
        });
    }

    Windows() {
        const all = global.get_window_actors().map(a => a.meta_window).filter(w => w);
        const sorted = global.display.sort_windows_by_stacking(all);
        return JSON.stringify(sorted.map(w => {
            const r = w.get_frame_rect();
            return {
                id: Number(w.get_id()),
                title: w.get_title() ?? '',
                class: w.get_wm_class() ?? '',
                app: w.get_gtk_application_id() ?? w.get_sandboxed_app_id() ?? '',
                pid: w.get_pid(),
                x: r.x,
                y: r.y,
                w: r.width,
                h: r.height,
                kind: w.get_window_type(),
                minimized: w.minimized,
                focused: w.has_focus(),
            };
        }));
    }
}
