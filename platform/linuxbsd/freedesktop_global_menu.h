// SPDX-License-Identifier: MIT
// The Linux desktop's global menu: the editor's menus exported over D-Bus as a
// `com.canonical.dbusmenu` object and registered with the desktop's `com.canonical.AppMenu.Registrar`,
// so a top bar that shows the focused app's menus (Nodalix's) shows Orbe's.
//
// A window on Wayland has no global id, so the registration names this process instead, and only a
// registrar that pairs a menu with its window by process can show it: Nodalix's, whose process also
// owns `space.unmade.GlobalMenu`. With any other registrar, or none, nothing is registered.

#pragma once

#ifdef DBUS_ENABLED

#include "core/os/mutex.h"
#include "core/os/thread.h"
#include "core/string/ustring.h"
#include "core/templates/safe_refcount.h"
#include "core/templates/vector.h"
#include "core/variant/callable.h"

struct DBusConnection;
struct DBusMessage;
struct DBusMessageIter;

class FreeDesktopGlobalMenu {
public:
	struct Row {
		int id = 0; // Numbered from 1; 0 is the root.
		String label;
		bool separator = false;
		bool enabled = true;
		int toggle = 0; // 0 none, 1 check mark, 2 radio.
		bool checked = false;
		bool submenu = false;
		String shortcut_key; // The key of the accelerator, as its name.
		Vector<String> shortcut_modifiers; // "Control", "Shift", "Alt", "Super".
		Vector<int> children;
	};

private:
	Thread thread;
	SafeFlag abort;
	SafeFlag registered;
	SafeFlag layout_changed;
	Mutex mutex;
	Vector<Row> rows; // Indexed by id - 1.
	Vector<int> root;
	uint32_t revision = 1;
	Callable on_activate; // Called on the main thread with the id of the row the person chose.
	Callable on_about_to_show;
	Callable on_registered;
	DBusConnection *connection = nullptr;

	static void _thread_func(void *p_ud);
	void _run();

	bool _register();
	void _handle(DBusMessage *p_message);
	void _reply_layout(DBusMessage *p_message, int p_parent, int p_depth);
	void _append_layout(DBusMessageIter *p_iter, int p_id, int p_depth);
	void _append_properties(DBusMessageIter *p_iter, int p_id);
	const Row *_row(int p_id) const;

public:
	// Starts exporting; false when there is no session bus. `p_activate(int)` runs on the main
	// thread when an item is chosen.
	bool start(const Callable &p_activate, const Callable &p_about_to_show, const Callable &p_registered);
	void stop();
	bool is_registered() const { return registered.is_set(); }

	// Replaces the menus: `p_root` lists the top-level rows.
	void set_rows(const Vector<Row> &p_rows, const Vector<int> &p_root);

	// True on a Wayland session with a session bus.
	static bool is_supported();

	~FreeDesktopGlobalMenu();
};

#endif // DBUS_ENABLED
