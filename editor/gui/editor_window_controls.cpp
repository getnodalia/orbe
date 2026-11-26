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

// NDL's window buttons are neutral at rest (the control background, secondary ink) and only take
// their colour while hovered: amber for minimize, the app's accent for maximize, red for close.
EditorWindowButton::Palette EditorWindowButton::_get_palette() const {
	Palette p;
	const Color base = EditorSettings::get_singleton() ? Color(EDITOR_GET("interface/theme/base_color")) : Color(0.11, 0.12, 0.16);
	const bool dark = base.get_luminance() < 0.5;
	p.rest_fill = dark ? Color::from_rgba8(29, 32, 41) : Color::from_rgba8(236, 238, 242);
	p.rest_ink = dark ? Color::from_rgba8(180, 184, 198) : Color::from_rgba8(68, 74, 92);
	p.pressed_fill = dark ? Color::from_rgba8(42, 46, 58) : Color::from_rgba8(218, 221, 228);
	switch (kind) {
		case KIND_MINIMIZE: {
			p.hover_fill = dark ? Color::from_rgba8(51, 36, 10) : Color::from_rgba8(251, 239, 208);
			p.hover_ink = dark ? Color::from_rgba8(242, 193, 78) : Color::from_rgba8(122, 79, 0);
		} break;
		case KIND_MAXIMIZE: {
			const Color accent = EditorSettings::get_singleton() ? Color(EDITOR_GET("interface/theme/accent_color")) : Color::hex(0xDDA10AFF);
			p.hover_fill = p.rest_fill.lerp(accent, dark ? 0.22 : 0.18);
			p.hover_ink = dark ? accent.lightened(0.15) : accent.darkened(0.5);
		} break;
		case KIND_CLOSE: {
			p.hover_fill = Color::from_rgba8(255, 92, 77);
			p.hover_ink = Color::from_rgba8(11, 12, 16);
		} break;
	}
	return p;
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
			const Palette pal = _get_palette();
			const Color fill = pressed ? pal.pressed_fill : (hovered ? pal.hover_fill : pal.rest_fill);
			const Color c = (hovered || pressed) ? pal.hover_ink : pal.rest_ink;
			const Size2 s = get_size();
			const Point2 mid = s / 2;
			const float radius = MIN(s.x, s.y) / 2;
			draw_circle(mid, radius, fill);

			const float g = 5.0 * MAX(1.0, EDSCALE); // Half the glyph's size.
			const float t = 4.0 * MAX(1.0, EDSCALE); // Length of a corner's arms.
			const float w = 1.8 * MAX(1.0, EDSCALE);
			switch (kind) {
				case KIND_MINIMIZE: {
					draw_line(mid + Vector2(-g, 0), mid + Vector2(g, 0), c, w, true);
				} break;
				case KIND_MAXIMIZE: {
					Window *win = get_window();
					const bool maximized = win && win->get_mode() == Window::MODE_MAXIMIZED;
					// Two diagonal corners, pointing out to maximize and in to restore.
					if (maximized) {
						const float o = 1.5 * MAX(1.0, EDSCALE);
						const Vector2 a = mid + Vector2(o, -o); // Top right corner, its point turned to the centre.
						draw_polyline(Vector<Vector2>({ a + Vector2(t, 0), a, a + Vector2(0, -t) }), c, w, true);
						const Vector2 b = mid + Vector2(-o, o);
						draw_polyline(Vector<Vector2>({ b + Vector2(-t, 0), b, b + Vector2(0, t) }), c, w, true);
					} else {
						const Vector2 a = mid + Vector2(g, -g);
						draw_polyline(Vector<Vector2>({ a + Vector2(-t, 0), a, a + Vector2(0, t) }), c, w, true);
						const Vector2 b = mid + Vector2(-g, g);
						draw_polyline(Vector<Vector2>({ b + Vector2(t, 0), b, b + Vector2(0, -t) }), c, w, true);
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
