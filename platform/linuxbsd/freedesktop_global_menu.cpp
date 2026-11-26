// SPDX-License-Identifier: MIT

#include "freedesktop_global_menu.h"

#ifdef DBUS_ENABLED

#include "core/os/os.h"

#ifdef SOWRAP_ENABLED
#include "dbus-so_wrap.h"
#else
#include <dbus/dbus.h>
#endif

#include <unistd.h>

namespace {
const char *REGISTRAR = "com.canonical.AppMenu.Registrar";
const char *REGISTRAR_PATH = "/com/canonical/AppMenu/Registrar";
const char *NODALIX_MENU = "space.unmade.GlobalMenu";
const char *MENU_PATH = "/dev/danielmiguelt/Orbe/GlobalMenu";
const char *MENU_IFACE = "com.canonical.dbusmenu";

String owner_of(DBusConnection *p_conn, const char *p_name) {
	DBusMessage *msg = dbus_message_new_method_call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetNameOwner");
	if (!msg) {
		return String();
	}
	dbus_message_append_args(msg, DBUS_TYPE_STRING, &p_name, DBUS_TYPE_INVALID);
	DBusError err;
	dbus_error_init(&err);
	DBusMessage *reply = dbus_connection_send_with_reply_and_block(p_conn, msg, 200, &err);
	dbus_message_unref(msg);
	String owner;
	if (dbus_error_is_set(&err)) {
		dbus_error_free(&err);
	} else if (reply) {
		const char *s = nullptr;
		if (dbus_message_get_args(reply, nullptr, DBUS_TYPE_STRING, &s, DBUS_TYPE_INVALID) && s) {
			owner = String::utf8(s);
		}
	}
	if (reply) {
		dbus_message_unref(reply);
	}
	return owner;
}

void put_string(DBusMessageIter *p_dict, const char *p_key, const char *p_value) {
	DBusMessageIter entry, variant;
	dbus_message_iter_open_container(p_dict, DBUS_TYPE_DICT_ENTRY, nullptr, &entry);
	dbus_message_iter_append_basic(&entry, DBUS_TYPE_STRING, &p_key);
	dbus_message_iter_open_container(&entry, DBUS_TYPE_VARIANT, "s", &variant);
	dbus_message_iter_append_basic(&variant, DBUS_TYPE_STRING, &p_value);
	dbus_message_iter_close_container(&entry, &variant);
	dbus_message_iter_close_container(p_dict, &entry);
}

void put_bool(DBusMessageIter *p_dict, const char *p_key, bool p_value) {
	dbus_bool_t v = p_value;
	DBusMessageIter entry, variant;
	dbus_message_iter_open_container(p_dict, DBUS_TYPE_DICT_ENTRY, nullptr, &entry);
	dbus_message_iter_append_basic(&entry, DBUS_TYPE_STRING, &p_key);
	dbus_message_iter_open_container(&entry, DBUS_TYPE_VARIANT, "b", &variant);
	dbus_message_iter_append_basic(&variant, DBUS_TYPE_BOOLEAN, &v);
	dbus_message_iter_close_container(&entry, &variant);
	dbus_message_iter_close_container(p_dict, &entry);
}

void put_int(DBusMessageIter *p_dict, const char *p_key, int32_t p_value) {
	DBusMessageIter entry, variant;
	dbus_message_iter_open_container(p_dict, DBUS_TYPE_DICT_ENTRY, nullptr, &entry);
	dbus_message_iter_append_basic(&entry, DBUS_TYPE_STRING, &p_key);
	dbus_message_iter_open_container(&entry, DBUS_TYPE_VARIANT, "i", &variant);
	dbus_message_iter_append_basic(&variant, DBUS_TYPE_INT32, &p_value);
	dbus_message_iter_close_container(&entry, &variant);
	dbus_message_iter_close_container(p_dict, &entry);
}

// "shortcut" is an array of arrays of strings: one chord, the modifiers and then the key.
void put_shortcut(DBusMessageIter *p_dict, const Vector<String> &p_modifiers, const String &p_key) {
	const char *key = "shortcut";
	DBusMessageIter entry, variant, chords, chord;
	dbus_message_iter_open_container(p_dict, DBUS_TYPE_DICT_ENTRY, nullptr, &entry);
	dbus_message_iter_append_basic(&entry, DBUS_TYPE_STRING, &key);
	dbus_message_iter_open_container(&entry, DBUS_TYPE_VARIANT, "aas", &variant);
	dbus_message_iter_open_container(&variant, DBUS_TYPE_ARRAY, "as", &chords);
	dbus_message_iter_open_container(&chords, DBUS_TYPE_ARRAY, "s", &chord);
	for (const String &m : p_modifiers) {
		CharString u = m.utf8();
		const char *s = u.get_data();
		dbus_message_iter_append_basic(&chord, DBUS_TYPE_STRING, &s);
	}
	CharString k = p_key.utf8();
	const char *ks = k.get_data();
	dbus_message_iter_append_basic(&chord, DBUS_TYPE_STRING, &ks);
	dbus_message_iter_close_container(&chords, &chord);
	dbus_message_iter_close_container(&variant, &chords);
	dbus_message_iter_close_container(&entry, &variant);
	dbus_message_iter_close_container(p_dict, &entry);
}
} // namespace

bool FreeDesktopGlobalMenu::is_supported() {
	return OS::get_singleton()->get_environment("WAYLAND_DISPLAY") != String() || OS::get_singleton()->get_environment("XDG_SESSION_TYPE") == "wayland";
}

const FreeDesktopGlobalMenu::Row *FreeDesktopGlobalMenu::_row(int p_id) const {
	if (p_id < 1 || p_id > rows.size()) {
		return nullptr;
	}
	return &rows[p_id - 1];
}

void FreeDesktopGlobalMenu::_append_properties(DBusMessageIter *p_iter, int p_id) {
	DBusMessageIter dict;
	dbus_message_iter_open_container(p_iter, DBUS_TYPE_ARRAY, "{sv}", &dict);
	const Row *row = _row(p_id);
	if (row) {
		if (row->separator) {
			put_string(&dict, "type", "separator");
		} else {
			CharString label = row->label.utf8();
			put_string(&dict, "label", label.get_data());
			put_bool(&dict, "enabled", row->enabled);
			if (row->submenu) {
				put_string(&dict, "children-display", "submenu");
			}
			if (row->toggle == 1) {
				put_string(&dict, "toggle-type", "checkmark");
				put_int(&dict, "toggle-state", row->checked ? 1 : 0);
			} else if (row->toggle == 2) {
				put_string(&dict, "toggle-type", "radio");
				put_int(&dict, "toggle-state", row->checked ? 1 : 0);
			}
			if (!row->shortcut_key.is_empty()) {
				put_shortcut(&dict, row->shortcut_modifiers, row->shortcut_key);
			}
		}
	} else if (p_id == 0) {
		put_string(&dict, "children-display", "submenu");
	}
	dbus_message_iter_close_container(p_iter, &dict);
}

// (i a{sv} av): the id, its properties and its children, each a variant holding the same.
void FreeDesktopGlobalMenu::_append_layout(DBusMessageIter *p_iter, int p_id, int p_depth) {
	DBusMessageIter item, children;
	dbus_message_iter_open_container(p_iter, DBUS_TYPE_STRUCT, nullptr, &item);
	int32_t id = p_id;
	dbus_message_iter_append_basic(&item, DBUS_TYPE_INT32, &id);
	_append_properties(&item, p_id);
	dbus_message_iter_open_container(&item, DBUS_TYPE_ARRAY, "v", &children);
	if (p_depth != 0) {
		const Vector<int> &kids = p_id == 0 ? root : (_row(p_id) ? _row(p_id)->children : Vector<int>());
		for (int child : kids) {
			DBusMessageIter variant;
			dbus_message_iter_open_container(&children, DBUS_TYPE_VARIANT, "(ia{sv}av)", &variant);
			_append_layout(&variant, child, p_depth < 0 ? p_depth : p_depth - 1);
			dbus_message_iter_close_container(&children, &variant);
		}
	}
	dbus_message_iter_close_container(&item, &children);
	dbus_message_iter_close_container(p_iter, &item);
}

void FreeDesktopGlobalMenu::_reply_layout(DBusMessage *p_message, int p_parent, int p_depth) {
	DBusMessage *reply = dbus_message_new_method_return(p_message);
	if (!reply) {
		return;
	}
	DBusMessageIter it;
	dbus_message_iter_init_append(reply, &it);
	uint32_t rev = revision;
	dbus_message_iter_append_basic(&it, DBUS_TYPE_UINT32, &rev);
	_append_layout(&it, p_parent, p_depth);
	dbus_connection_send(connection, reply, nullptr);
	dbus_message_unref(reply);
}

void FreeDesktopGlobalMenu::_handle(DBusMessage *p_message) {
	const char *iface = dbus_message_get_interface(p_message);
	const char *member = dbus_message_get_member(p_message);
	if (!member) {
		return;
	}
	const String m = String::utf8(member);
	const String i = iface ? String::utf8(iface) : String();

	if (i == MENU_IFACE) {
		MutexLock lock(mutex);
		if (m == "GetLayout") {
			int32_t parent = 0, depth = -1;
			dbus_message_get_args(p_message, nullptr, DBUS_TYPE_INT32, &parent, DBUS_TYPE_INT32, &depth, DBUS_TYPE_INVALID);
			_reply_layout(p_message, parent, depth);
		} else if (m == "GetGroupProperties") {
			DBusMessageIter in;
			dbus_message_iter_init(p_message, &in);
			Vector<int> ids;
			if (dbus_message_iter_get_arg_type(&in) == DBUS_TYPE_ARRAY) {
				DBusMessageIter arr;
				dbus_message_iter_recurse(&in, &arr);
				while (dbus_message_iter_get_arg_type(&arr) == DBUS_TYPE_INT32) {
					int32_t v = 0;
					dbus_message_iter_get_basic(&arr, &v);
					ids.push_back(v);
					dbus_message_iter_next(&arr);
				}
			}
			DBusMessage *reply = dbus_message_new_method_return(p_message);
			DBusMessageIter out, list;
			dbus_message_iter_init_append(reply, &out);
			dbus_message_iter_open_container(&out, DBUS_TYPE_ARRAY, "(ia{sv})", &list);
			for (int id : ids) {
				DBusMessageIter st;
				dbus_message_iter_open_container(&list, DBUS_TYPE_STRUCT, nullptr, &st);
				int32_t v = id;
				dbus_message_iter_append_basic(&st, DBUS_TYPE_INT32, &v);
				_append_properties(&st, id);
				dbus_message_iter_close_container(&list, &st);
			}
			dbus_message_iter_close_container(&out, &list);
			dbus_connection_send(connection, reply, nullptr);
			dbus_message_unref(reply);
		} else if (m == "GetProperty") {
			DBusMessage *reply = dbus_message_new_error(p_message, "org.freedesktop.DBus.Error.UnknownProperty", "Use GetGroupProperties");
			dbus_connection_send(connection, reply, nullptr);
			dbus_message_unref(reply);
		} else if (m == "Event") {
			int32_t id = 0;
			const char *event = nullptr;
			dbus_message_get_args(p_message, nullptr, DBUS_TYPE_INT32, &id, DBUS_TYPE_STRING, &event, DBUS_TYPE_INVALID);
			DBusMessage *reply = dbus_message_new_method_return(p_message);
			dbus_connection_send(connection, reply, nullptr);
			dbus_message_unref(reply);
			if (event && String::utf8(event) == "clicked" && on_activate.is_valid()) {
				on_activate.call_deferred(int(id));
			}
		} else if (m == "EventGroup") {
			DBusMessageIter in;
			dbus_message_iter_init(p_message, &in);
			if (dbus_message_iter_get_arg_type(&in) == DBUS_TYPE_ARRAY) {
				DBusMessageIter arr;
				dbus_message_iter_recurse(&in, &arr);
				while (dbus_message_iter_get_arg_type(&arr) == DBUS_TYPE_STRUCT) {
					DBusMessageIter st;
					dbus_message_iter_recurse(&arr, &st);
					int32_t id = 0;
					const char *event = nullptr;
					dbus_message_iter_get_basic(&st, &id);
					dbus_message_iter_next(&st);
					dbus_message_iter_get_basic(&st, &event);
					if (event && String::utf8(event) == "clicked" && on_activate.is_valid()) {
						on_activate.call_deferred(int(id));
					}
					dbus_message_iter_next(&arr);
				}
			}
			DBusMessage *reply = dbus_message_new_method_return(p_message);
			DBusMessageIter out, list;
			dbus_message_iter_init_append(reply, &out);
			dbus_message_iter_open_container(&out, DBUS_TYPE_ARRAY, "i", &list);
			dbus_message_iter_close_container(&out, &list);
			dbus_connection_send(connection, reply, nullptr);
			dbus_message_unref(reply);
		} else if (m == "AboutToShow") {
			DBusMessage *reply = dbus_message_new_method_return(p_message);
			dbus_bool_t need = FALSE;
			dbus_message_append_args(reply, DBUS_TYPE_BOOLEAN, &need, DBUS_TYPE_INVALID);
			dbus_connection_send(connection, reply, nullptr);
			dbus_message_unref(reply);
			if (on_about_to_show.is_valid()) {
				on_about_to_show.call_deferred();
			}
		} else if (m == "AboutToShowGroup") {
			DBusMessage *reply = dbus_message_new_method_return(p_message);
			DBusMessageIter out, a, b;
			dbus_message_iter_init_append(reply, &out);
			dbus_message_iter_open_container(&out, DBUS_TYPE_STRUCT, nullptr, &b);
			dbus_message_iter_open_container(&b, DBUS_TYPE_ARRAY, "i", &a);
			dbus_message_iter_close_container(&b, &a);
			dbus_message_iter_open_container(&b, DBUS_TYPE_ARRAY, "i", &a);
			dbus_message_iter_close_container(&b, &a);
			dbus_message_iter_close_container(&out, &b);
			dbus_connection_send(connection, reply, nullptr);
			dbus_message_unref(reply);
			if (on_about_to_show.is_valid()) {
				on_about_to_show.call_deferred();
			}
		}
		return;
	}

	if (i == "org.freedesktop.DBus.Properties") {
		DBusMessage *reply = dbus_message_new_method_return(p_message);
		DBusMessageIter out, dict;
		dbus_message_iter_init_append(reply, &out);
		if (m == "GetAll") {
			dbus_message_iter_open_container(&out, DBUS_TYPE_ARRAY, "{sv}", &dict);
			DBusMessageIter entry, variant;
			const char *k = "Version";
			uint32_t version = 3;
			dbus_message_iter_open_container(&dict, DBUS_TYPE_DICT_ENTRY, nullptr, &entry);
			dbus_message_iter_append_basic(&entry, DBUS_TYPE_STRING, &k);
			dbus_message_iter_open_container(&entry, DBUS_TYPE_VARIANT, "u", &variant);
			dbus_message_iter_append_basic(&variant, DBUS_TYPE_UINT32, &version);
			dbus_message_iter_close_container(&entry, &variant);
			dbus_message_iter_close_container(&dict, &entry);
			put_string(&dict, "TextDirection", "ltr");
			put_string(&dict, "Status", "normal");
			dbus_message_iter_close_container(&out, &dict);
		} else if (m == "Get") {
			const char *prop = nullptr;
			const char *ifc = nullptr;
			dbus_message_get_args(p_message, nullptr, DBUS_TYPE_STRING, &ifc, DBUS_TYPE_STRING, &prop, DBUS_TYPE_INVALID);
			DBusMessageIter variant;
			const String p = prop ? String::utf8(prop) : String();
			if (p == "Version") {
				uint32_t version = 3;
				dbus_message_iter_open_container(&out, DBUS_TYPE_VARIANT, "u", &variant);
				dbus_message_iter_append_basic(&variant, DBUS_TYPE_UINT32, &version);
				dbus_message_iter_close_container(&out, &variant);
			} else {
				const char *v = p == "TextDirection" ? "ltr" : "normal";
				dbus_message_iter_open_container(&out, DBUS_TYPE_VARIANT, "s", &variant);
				dbus_message_iter_append_basic(&variant, DBUS_TYPE_STRING, &v);
				dbus_message_iter_close_container(&out, &variant);
			}
		}
		dbus_connection_send(connection, reply, nullptr);
		dbus_message_unref(reply);
		return;
	}

	if (dbus_message_get_type(p_message) == DBUS_MESSAGE_TYPE_METHOD_CALL) {
		DBusMessage *reply = dbus_message_new_error(p_message, "org.freedesktop.DBus.Error.UnknownMethod", "Unknown method");
		dbus_connection_send(connection, reply, nullptr);
		dbus_message_unref(reply);
	}
}

bool FreeDesktopGlobalMenu::_register() {
	// Only Nodalix's registrar finds a Wayland window's menu by its process.
	const String registrar = owner_of(connection, REGISTRAR);
	const String nodalix = owner_of(connection, NODALIX_MENU);
	if (registrar.is_empty() || registrar != nodalix) {
		return false;
	}
	DBusMessage *msg = dbus_message_new_method_call(REGISTRAR, REGISTRAR_PATH, REGISTRAR, "RegisterWindow");
	if (!msg) {
		return false;
	}
	uint32_t pid = getpid();
	const char *path = MENU_PATH;
	dbus_message_append_args(msg, DBUS_TYPE_UINT32, &pid, DBUS_TYPE_OBJECT_PATH, &path, DBUS_TYPE_INVALID);
	DBusError err;
	dbus_error_init(&err);
	DBusMessage *reply = dbus_connection_send_with_reply_and_block(connection, msg, 500, &err);
	dbus_message_unref(msg);
	bool ok = !dbus_error_is_set(&err) && reply;
	if (dbus_error_is_set(&err)) {
		dbus_error_free(&err);
	}
	if (reply) {
		dbus_message_unref(reply);
	}
	return ok;
}

void FreeDesktopGlobalMenu::_run() {
	DBusError err;
	dbus_error_init(&err);
	connection = dbus_bus_get_private(DBUS_BUS_SESSION, &err);
	if (dbus_error_is_set(&err) || !connection) {
		dbus_error_free(&err);
		connection = nullptr;
		return;
	}
	dbus_connection_set_exit_on_disconnect(connection, FALSE);
	dbus_bus_add_match(connection, "type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged'", &err);
	if (dbus_error_is_set(&err)) {
		dbus_error_free(&err);
	}

	const bool now = _register();
	registered.set_to(now);
	if (now && on_registered.is_valid()) {
		on_registered.call_deferred(true);
	}

	while (!abort.is_set()) {
		dbus_connection_read_write(connection, 100);
		while (DBusMessage *msg = dbus_connection_pop_message(connection)) {
			const char *path = dbus_message_get_path(msg);
			if (dbus_message_get_type(msg) == DBUS_MESSAGE_TYPE_METHOD_CALL && path && String::utf8(path) == MENU_PATH) {
				_handle(msg);
			} else if (dbus_message_is_signal(msg, "org.freedesktop.DBus", "NameOwnerChanged")) {
				const char *name = nullptr;
				const char *old_owner = nullptr;
				const char *new_owner = nullptr;
				if (dbus_message_get_args(msg, nullptr, DBUS_TYPE_STRING, &name, DBUS_TYPE_STRING, &old_owner, DBUS_TYPE_STRING, &new_owner, DBUS_TYPE_INVALID) && name && new_owner && *new_owner) {
					const String n = String::utf8(name);
					if (n == REGISTRAR || n == NODALIX_MENU) {
						const bool ok = _register();
						if (ok != registered.is_set()) {
							registered.set_to(ok);
							if (on_registered.is_valid()) {
								on_registered.call_deferred(ok);
							}
						}
					}
				}
			}
			dbus_message_unref(msg);
		}
		if (layout_changed.is_set()) {
			layout_changed.clear();
			DBusMessage *sig = dbus_message_new_signal(MENU_PATH, MENU_IFACE, "LayoutUpdated");
			if (sig) {
				uint32_t rev;
				{
					MutexLock lock(mutex);
					rev = revision;
				}
				int32_t parent = 0;
				dbus_message_append_args(sig, DBUS_TYPE_UINT32, &rev, DBUS_TYPE_INT32, &parent, DBUS_TYPE_INVALID);
				dbus_connection_send(connection, sig, nullptr);
				dbus_message_unref(sig);
			}
		}
	}

	dbus_connection_close(connection);
	dbus_connection_unref(connection);
	connection = nullptr;
}

void FreeDesktopGlobalMenu::_thread_func(void *p_ud) {
	FreeDesktopGlobalMenu *self = static_cast<FreeDesktopGlobalMenu *>(p_ud);
	self->_run();
}

bool FreeDesktopGlobalMenu::start(const Callable &p_activate, const Callable &p_about_to_show, const Callable &p_registered) {
	if (thread.is_started()) {
		return true;
	}
	on_activate = p_activate;
	on_about_to_show = p_about_to_show;
	on_registered = p_registered;
	dbus_threads_init_default();
	abort.clear();
	thread.start(FreeDesktopGlobalMenu::_thread_func, this);
	return true;
}

void FreeDesktopGlobalMenu::stop() {
	abort.set();
	if (thread.is_started()) {
		thread.wait_to_finish();
	}
	registered.clear();
}

void FreeDesktopGlobalMenu::set_rows(const Vector<Row> &p_rows, const Vector<int> &p_root) {
	{
		MutexLock lock(mutex);
		revision++;
		rows = p_rows;
		root = p_root;
	}
	layout_changed.set();
}

FreeDesktopGlobalMenu::~FreeDesktopGlobalMenu() {
	stop();
}

#endif // DBUS_ENABLED
