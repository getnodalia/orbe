// SPDX-License-Identifier: MIT

#include "editor_window_controls.h"

#include "core/object/callable_mp.h"
#include "core/os/os.h"
#include "editor/gui/editor_title_bar.h"
#include "editor/settings/editor_settings.h"
#include "editor/themes/editor_scale.h"
#include "scene/main/scene_tree.h"
#include "scene/main/window.h"
#include "servers/display/display_server.h"

namespace {
constexpr int BUTTON_SIZE = 28;
constexpr int EDGE_SIZE = 5;
} // namespace

// The three colours are NDL's: close red, maximize cyan, minimize amber.
Color EditorWindowButton::_get_color() const {
	switch (kind) {
		case KIND_MINIMIZE:
			return Color::hex(0xF5A524FF);
		case KIND_MAXIMIZE:
			return Color::hex(0x22B8CFFF);
		case KIND_CLOSE:
			return Color::hex(0xE5484DFF);
	}
	return Color(1, 1, 1);
}

void EditorWindowButton::_bind_methods() {
	ADD_SIGNAL(MethodInfo("pressed"));
}

void EditorWindowButton::set_kind(Kind p_kind) {
	kind = p_kind;
	queue_redraw();
}

void EditorWindowButton::gui_input(const Ref<InputEvent> &p_event) {
	Ref<InputEventMouseButton> mb = p_event;
	if (mb.is_valid() && mb->get_button_index() == MouseButton::LEFT) {
		if (mb->is_pressed()) {
			pressed = true;
		} else {
			if (pressed && hovered) {
				emit_signal("pressed");
			}
			pressed = false;
		}
		queue_redraw();
		accept_event();
	}
}

void EditorWindowButton::_notification(int p_what) {
	switch (p_what) {
		case NOTIFICATION_MOUSE_ENTER: {
			hovered = true;
			queue_redraw();
		} break;
		case NOTIFICATION_MOUSE_EXIT: {
			hovered = false;
			pressed = false;
			queue_redraw();
		} break;
		case NOTIFICATION_DRAW: {
			const Color c = _get_color();
			const Size2 s = get_size();
			const Point2 mid = s / 2;
			const float radius = MIN(s.x, s.y) / 2;
			Color fill = c;
			fill.a = pressed ? 0.45 : (hovered ? 0.34 : 0.18);
			draw_circle(mid, radius, fill);

			const float g = 5.0; // Half the glyph's size.
			const float w = 1.6 * MAX(1.0, EDSCALE);
			switch (kind) {
				case KIND_MINIMIZE: {
					draw_line(mid + Vector2(-g, 0), mid + Vector2(g, 0), c, w, true);
				} break;
				case KIND_MAXIMIZE: {
					Window *win = get_window();
					if (win && win->get_mode() == Window::MODE_MAXIMIZED) {
						// Restore: two squares, one behind the other.
						draw_rect(Rect2(mid + Vector2(-g, -g + 2), Size2(2 * g - 2, 2 * g - 2)), c, false, w);
						draw_line(mid + Vector2(-g + 2, -g + 2), mid + Vector2(-g + 2, -g), c, w, true);
						draw_line(mid + Vector2(-g + 2, -g), mid + Vector2(g, -g), c, w, true);
						draw_line(mid + Vector2(g, -g), mid + Vector2(g, g - 2), c, w, true);
						draw_line(mid + Vector2(g, g - 2), mid + Vector2(g - 2, g - 2), c, w, true);
					} else {
						draw_rect(Rect2(mid + Vector2(-g, -g), Size2(2 * g, 2 * g)), c, false, w);
					}
				} break;
				case KIND_CLOSE: {
					draw_line(mid + Vector2(-g, -g), mid + Vector2(g, g), c, w, true);
					draw_line(mid + Vector2(-g, g), mid + Vector2(g, -g), c, w, true);
				} break;
			}
		} break;
	}
}

EditorWindowButton::EditorWindowButton() {
	set_custom_minimum_size(Size2(BUTTON_SIZE, BUTTON_SIZE) * EDSCALE);
	set_v_size_flags(Control::SIZE_SHRINK_CENTER);
	set_mouse_filter(Control::MOUSE_FILTER_STOP);
	set_focus_mode(Control::FOCUS_NONE);
}

void EditorWindowEdge::setup(DisplayServerEnums::WindowResizeEdge p_edge, Control::CursorShape p_cursor) {
	edge = p_edge;
	set_default_cursor_shape(p_cursor);
	set_mouse_filter(Control::MOUSE_FILTER_STOP);
	set_focus_mode(Control::FOCUS_NONE);
}

void EditorWindowEdge::gui_input(const Ref<InputEvent> &p_event) {
	Ref<InputEventMouseButton> mb = p_event;
	if (mb.is_valid() && mb->get_button_index() == MouseButton::LEFT && mb->is_pressed()) {
		Window *w = get_window();
		if (w && w->get_mode() == Window::MODE_WINDOWED) {
			DisplayServer::get_singleton()->window_start_resize(edge, w->get_window_id());
			accept_event();
		}
	}
}

bool EditorWindowControls::is_enabled() {
#ifdef LINUXBSD_ENABLED
	if (!EditorSettings::get_singleton()) {
		return false;
	}
	DisplayServer *ds = DisplayServer::get_singleton();
	if (!ds || !ds->has_feature(DisplayServerEnums::FEATURE_WINDOW_DRAG)) {
		return false; // Without the window manager's help the frame cannot be moved or resized.
	}
	return !bool(EDITOR_GET("interface/editor/appearance/use_system_window_controls"));
#else
	return false; // macOS and Windows always keep their own controls.
#endif
}

void EditorWindowControls::_window_changed() {
	Window *w = get_window();
	const bool windowed = w && w->get_mode() == Window::MODE_WINDOWED;
	for (EditorWindowEdge *e : edges) {
		e->set_visible(windowed);
	}
	if (maximize) {
		maximize->queue_redraw();
	}
}

void EditorWindowControls::_minimize() {
	Window *w = get_window();
	if (w) {
		w->set_mode(Window::MODE_MINIMIZED);
	}
}

void EditorWindowControls::_toggle_maximize() {
	Window *w = get_window();
	if (w) {
		w->set_mode(w->get_mode() == Window::MODE_MAXIMIZED ? Window::MODE_WINDOWED : Window::MODE_MAXIMIZED);
	}
}

void EditorWindowControls::_request_close() {
	Window *w = get_window();
	if (w) {
		w->propagate_notification(NOTIFICATION_WM_CLOSE_REQUEST);
		w->emit_signal("close_requested");
	}
}

void EditorWindowControls::install(EditorTitleBar *p_title_bar, Control *p_edges_parent) {
	if (!is_enabled() || !p_title_bar || !p_edges_parent) {
		return;
	}

	EditorWindowControls *controls = memnew(EditorWindowControls);
	controls->set_mouse_filter(Control::MOUSE_FILTER_PASS);
	controls->add_theme_constant_override("separation", 8 * EDSCALE);

	EditorWindowButton *minimize = memnew(EditorWindowButton);
	minimize->set_kind(EditorWindowButton::KIND_MINIMIZE);
	minimize->set_accessibility_name(TTRC("Minimize"));
	controls->add_child(minimize);

	controls->maximize = memnew(EditorWindowButton);
	controls->maximize->set_kind(EditorWindowButton::KIND_MAXIMIZE);
	controls->maximize->set_accessibility_name(TTRC("Maximize"));
	controls->add_child(controls->maximize);

	EditorWindowButton *close = memnew(EditorWindowButton);
	close->set_kind(EditorWindowButton::KIND_CLOSE);
	close->set_accessibility_name(TTRC("Close"));
	controls->add_child(close);

	p_title_bar->add_child(controls);
	p_title_bar->set_can_move_window(true);

	// The window loses its frame: the bar is what is left to grab.
	DisplayServer::get_singleton()->window_set_flag(DisplayServerEnums::WINDOW_FLAG_BORDERLESS, true, DisplayServerEnums::MAIN_WINDOW_ID);

	struct EdgeSpec {
		DisplayServerEnums::WindowResizeEdge edge;
		Control::CursorShape cursor;
		Control::LayoutPreset preset;
	};
	// Corners are listed last so they sit above the sides.
	const EdgeSpec specs[] = {
		{ DisplayServerEnums::WINDOW_EDGE_TOP, Control::CURSOR_VSIZE, Control::PRESET_TOP_WIDE },
		{ DisplayServerEnums::WINDOW_EDGE_BOTTOM, Control::CURSOR_VSIZE, Control::PRESET_BOTTOM_WIDE },
		{ DisplayServerEnums::WINDOW_EDGE_LEFT, Control::CURSOR_HSIZE, Control::PRESET_LEFT_WIDE },
		{ DisplayServerEnums::WINDOW_EDGE_RIGHT, Control::CURSOR_HSIZE, Control::PRESET_RIGHT_WIDE },
		{ DisplayServerEnums::WINDOW_EDGE_TOP_LEFT, Control::CURSOR_FDIAGSIZE, Control::PRESET_TOP_LEFT },
		{ DisplayServerEnums::WINDOW_EDGE_TOP_RIGHT, Control::CURSOR_BDIAGSIZE, Control::PRESET_TOP_RIGHT },
		{ DisplayServerEnums::WINDOW_EDGE_BOTTOM_LEFT, Control::CURSOR_BDIAGSIZE, Control::PRESET_BOTTOM_LEFT },
		{ DisplayServerEnums::WINDOW_EDGE_BOTTOM_RIGHT, Control::CURSOR_FDIAGSIZE, Control::PRESET_BOTTOM_RIGHT },
	};
	const int edge_px = EDGE_SIZE * EDSCALE;
	const int corner_px = 2 * edge_px;
	for (const EdgeSpec &spec : specs) {
		EditorWindowEdge *e = memnew(EditorWindowEdge);
		e->setup(spec.edge, spec.cursor);
		p_edges_parent->add_child(e);
		const bool corner = spec.preset == Control::PRESET_TOP_LEFT || spec.preset == Control::PRESET_TOP_RIGHT || spec.preset == Control::PRESET_BOTTOM_LEFT || spec.preset == Control::PRESET_BOTTOM_RIGHT;
		e->set_anchors_and_offsets_preset(spec.preset, Control::PRESET_MODE_MINSIZE, 0);
		e->set_custom_minimum_size(corner ? Size2(corner_px, corner_px) : Size2(edge_px, edge_px));
		controls->edges.push_back(e);
	}

	Window *w = p_title_bar->get_window();
	if (w) {
		w->connect("size_changed", callable_mp(controls, &EditorWindowControls::_window_changed));
	}

	minimize->connect("pressed", callable_mp(controls, &EditorWindowControls::_minimize));
	controls->maximize->connect("pressed", callable_mp(controls, &EditorWindowControls::_toggle_maximize));
	close->connect("pressed", callable_mp(controls, &EditorWindowControls::_request_close));
}
