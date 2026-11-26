// SPDX-License-Identifier: MIT
// Orbe's window chrome on Linux, in the Nodalia Design Language: the round minimize, maximize and
// close buttons, and the edges that resize a window that has no frame of its own.

#pragma once

#include "scene/gui/box_container.h"
#include "servers/display/display_server_enums.h"

class EditorTitleBar;
class Window;

class EditorWindowButton : public Control {
	GDCLASS(EditorWindowButton, Control);

public:
	enum Kind {
		KIND_MINIMIZE,
		KIND_MAXIMIZE,
		KIND_CLOSE,
	};

private:
	Kind kind = KIND_CLOSE;
	bool hovered = false;
	bool pressed = false;

	struct Palette {
		Color rest_fill, rest_ink, pressed_fill, hover_fill, hover_ink;
	};
	Palette _get_palette() const;

protected:
	void _notification(int p_what);
	virtual void gui_input(const Ref<InputEvent> &p_event) override;
	static void _bind_methods();

public:
	void set_kind(Kind p_kind);
	EditorWindowButton();
};

class EditorWindowEdge : public Control {
	GDCLASS(EditorWindowEdge, Control);

	DisplayServerEnums::WindowResizeEdge edge = DisplayServerEnums::WINDOW_EDGE_BOTTOM_RIGHT;

protected:
	virtual void gui_input(const Ref<InputEvent> &p_event) override;
	static void _bind_methods() {}

public:
	void setup(DisplayServerEnums::WindowResizeEdge p_edge, Control::CursorShape p_cursor);
};

// Controls the window's decorations: on Linux, unless the person chose the desktop's own in the
// editor settings, the window is drawn without a frame and this adds what a frame gave.
class EditorWindowControls : public HBoxContainer {
	GDCLASS(EditorWindowControls, HBoxContainer);

	Vector<EditorWindowEdge *> edges;
	EditorWindowButton *maximize = nullptr;

	void _window_changed();
	void _minimize();
	void _toggle_maximize();
	void _request_close();

protected:
	static void _bind_methods() {}

public:
	// True when the window is drawn without a frame and shows these controls.
	static bool is_enabled();

	// Makes `p_title_bar`'s window frameless and adds the buttons to the end of the bar and the
	// resizing edges over `p_edges_parent`. Does nothing where `is_enabled()` is false.
	static void install(EditorTitleBar *p_title_bar, Control *p_edges_parent);
};
