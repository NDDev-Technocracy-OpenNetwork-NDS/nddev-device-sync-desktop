#include "my_application.h"

#include <flutter_linux/flutter_linux.h>
#ifdef GDK_WINDOWING_X11
#include <gdk/gdkx.h>
#endif

#include "flutter/generated_plugin_registrant.h"

struct _MyApplication {
  GtkApplication parent_instance;
  char** dart_entrypoint_arguments;
  bool quit_requested;
};

G_DEFINE_TYPE(MyApplication, my_application, GTK_TYPE_APPLICATION)

// Use Flutter's existing exit listener so Rust drains its owned work before
// GTK exits. Send through the public messenger without replacing its platform
// channel handler. JSONMethodCodec uses one result in a JSON list envelope.
static void quit_response_cb(GObject* object, GAsyncResult* result,
                             gpointer user_data) {
  g_autoptr(MyApplication) self = MY_APPLICATION(user_data);
  self->quit_requested = false;
  g_autoptr(GError) error = nullptr;
  g_autoptr(GBytes) message = fl_binary_messenger_send_on_channel_finish(
      FL_BINARY_MESSENGER(object), result, &error);
  g_autoptr(FlJsonMessageCodec) codec = fl_json_message_codec_new();
  g_autoptr(FlValue) envelope = message == nullptr ? nullptr :
      fl_message_codec_decode_message(FL_MESSAGE_CODEC(codec), message, &error);
  if (envelope == nullptr || fl_value_get_type(envelope) != FL_VALUE_TYPE_LIST ||
      fl_value_get_length(envelope) != 1) {
    g_warning("NDS application exit request failed");
    return;
  }
  FlValue* response = fl_value_get_list_value(envelope, 0);
  FlValue* decision = fl_value_get_type(response) == FL_VALUE_TYPE_MAP
      ? fl_value_lookup_string(response, "response") : nullptr;
  if (decision == nullptr || fl_value_get_type(decision) != FL_VALUE_TYPE_STRING) {
    g_warning("NDS application exit response invalid");
    return;
  }
  if (g_strcmp0(fl_value_get_string(decision), "exit") == 0) {
    g_application_quit(G_APPLICATION(self));
  }
}

static void request_quit(GApplication* application) {
  MyApplication* self = MY_APPLICATION(application);
  if (self->quit_requested) return;
  GList* windows = gtk_application_get_windows(GTK_APPLICATION(application));
  if (windows == nullptr) {
    g_application_quit(application);
    return;
  }
  GtkWidget* view = gtk_bin_get_child(GTK_BIN(windows->data));
  if (!FL_IS_VIEW(view)) return;
  g_autoptr(FlValue) call = fl_value_new_map();
  fl_value_set_string_take(call, "method", fl_value_new_string("System.requestAppExit"));
  g_autoptr(FlValue) args = fl_value_new_map();
  fl_value_set_string_take(args, "type", fl_value_new_string("cancelable"));
  fl_value_set_string_take(call, "args", fl_value_ref(args));
  g_autoptr(FlJsonMessageCodec) codec = fl_json_message_codec_new();
  g_autoptr(GBytes) message = fl_message_codec_encode_message(
      FL_MESSAGE_CODEC(codec), call, nullptr);
  if (message == nullptr) return;
  self->quit_requested = true;
  fl_binary_messenger_send_on_channel(
      fl_engine_get_binary_messenger(fl_view_get_engine(FL_VIEW(view))),
      "flutter/platform", message, nullptr, quit_response_cb, g_object_ref(self));
}

// Called when first Flutter frame received.
static void first_frame_cb(MyApplication* self, FlView* view) {
  gtk_widget_show(gtk_widget_get_toplevel(GTK_WIDGET(view)));
}

// Implements GApplication::activate.
static void my_application_activate(GApplication* application) {
  MyApplication* self = MY_APPLICATION(application);
  GList* windows = gtk_application_get_windows(GTK_APPLICATION(application));
  if (windows != nullptr) {
    gtk_window_present(GTK_WINDOW(windows->data));
    return;
  }
  GtkWindow* window =
      GTK_WINDOW(gtk_application_window_new(GTK_APPLICATION(application)));
  // Closing presentation leaves the existing Flutter engine and Rust runtime
  // alive. Activation reuses this same window instead of starting another one.
  g_signal_connect(window, "delete-event",
                   G_CALLBACK(gtk_widget_hide_on_delete), nullptr);
  gtk_window_set_title(window, "NDDev OpenNetwork");

  // Use a header bar when running in GNOME as this is the common style used
  // by applications and is the setup most users will be using (e.g. Ubuntu
  // desktop).
  // If running on X and not using GNOME then just use a traditional title bar
  // in case the window manager does more exotic layout, e.g. tiling.
  // If running on Wayland assume the header bar will work (may need changing
  // if future cases occur).
  gboolean use_header_bar = TRUE;
#ifdef GDK_WINDOWING_X11
  GdkScreen* screen = gtk_window_get_screen(window);
  if (GDK_IS_X11_SCREEN(screen)) {
    const gchar* wm_name = gdk_x11_screen_get_window_manager_name(screen);
    if (g_strcmp0(wm_name, "GNOME Shell") != 0) {
      use_header_bar = FALSE;
    }
  }
#endif
  if (use_header_bar) {
    GtkHeaderBar* header_bar = GTK_HEADER_BAR(gtk_header_bar_new());
    gtk_widget_show(GTK_WIDGET(header_bar));
    gtk_header_bar_set_title(header_bar, "NDDev OpenNetwork");
    gtk_header_bar_set_show_close_button(header_bar, TRUE);
    gtk_window_set_titlebar(window, GTK_WIDGET(header_bar));
  }

  g_autoptr(GdkPixbuf) icon =
      gdk_pixbuf_new_from_resource("/ai/nddev/nds/app_icon.png", nullptr);
  gtk_window_set_icon(window, icon);
  gtk_window_set_default_size(window, 1280, 720);

  g_autoptr(FlDartProject) project = fl_dart_project_new();
  // Use the supported Skia GPU backend for the Linux alpha under concurrent
  // GPU workloads. ponytail: requalify Impeller's framebuffer allocation after
  // a locked engine update passes low-VRAM native interaction acceptance.
  fl_dart_project_set_enable_impeller(project, FALSE);
  fl_dart_project_set_dart_entrypoint_arguments(
      project, self->dart_entrypoint_arguments);

  FlView* view = fl_view_new(project);
  GdkRGBA background_color;
  // Background defaults to black, override it here if necessary, e.g. #00000000
  // for transparent.
  gdk_rgba_parse(&background_color, "#000000");
  fl_view_set_background_color(view, &background_color);
  gtk_widget_show(GTK_WIDGET(view));
  gtk_container_add(GTK_CONTAINER(window), GTK_WIDGET(view));

  // Show the window when Flutter renders.
  // Requires the view to be realized so we can start rendering.
  g_signal_connect_swapped(view, "first-frame", G_CALLBACK(first_frame_cb),
                           self);
  gtk_widget_realize(GTK_WIDGET(view));

  fl_register_plugins(FL_PLUGIN_REGISTRY(view));

  gtk_widget_grab_focus(GTK_WIDGET(view));
}

// Implements GApplication::local_command_line.
static gboolean my_application_local_command_line(GApplication* application,
                                                  gchar*** arguments,
                                                  int* exit_status) {
  MyApplication* self = MY_APPLICATION(application);
  // Strip out the first argument as it is the binary name.
  self->dart_entrypoint_arguments = g_strdupv(*arguments + 1);

  g_autoptr(GError) error = nullptr;
  if (!g_application_register(application, nullptr, &error)) {
    g_warning("Failed to register: %s", error->message);
    *exit_status = 1;
    return TRUE;
  }

  g_application_activate(application);
  *exit_status = 0;

  return TRUE;
}

// Implements GApplication::startup.
static void my_application_startup(GApplication* application) {
  G_APPLICATION_CLASS(my_application_parent_class)->startup(application);

  g_autoptr(GSimpleAction) quit = g_simple_action_new("quit", nullptr);
  g_signal_connect_swapped(quit, "activate", G_CALLBACK(request_quit),
                           application);
  g_action_map_add_action(G_ACTION_MAP(application), G_ACTION(quit));
  const gchar* quit_accelerators[] = {"<Primary>q", nullptr};
  gtk_application_set_accels_for_action(GTK_APPLICATION(application), "app.quit",
                                        quit_accelerators);
}

// Implements GApplication::shutdown.
static void my_application_shutdown(GApplication* application) {
  // MyApplication* self = MY_APPLICATION(object);

  // Perform any actions required at application shutdown.

  G_APPLICATION_CLASS(my_application_parent_class)->shutdown(application);
}

// Implements GObject::dispose.
static void my_application_dispose(GObject* object) {
  MyApplication* self = MY_APPLICATION(object);
  g_clear_pointer(&self->dart_entrypoint_arguments, g_strfreev);
  G_OBJECT_CLASS(my_application_parent_class)->dispose(object);
}

static void my_application_class_init(MyApplicationClass* klass) {
  G_APPLICATION_CLASS(klass)->activate = my_application_activate;
  G_APPLICATION_CLASS(klass)->local_command_line =
      my_application_local_command_line;
  G_APPLICATION_CLASS(klass)->startup = my_application_startup;
  G_APPLICATION_CLASS(klass)->shutdown = my_application_shutdown;
  G_OBJECT_CLASS(klass)->dispose = my_application_dispose;
}

static void my_application_init(MyApplication* self) {
  self->quit_requested = false;
}

MyApplication* my_application_new() {
  // Set the program name to the application ID, which helps various systems
  // like GTK and desktop environments map this running application to its
  // corresponding .desktop file. This ensures better integration by allowing
  // the application to be recognized beyond its binary name.
  g_set_prgname(APPLICATION_ID);

  return MY_APPLICATION(g_object_new(my_application_get_type(),
                                     "application-id", APPLICATION_ID, "flags",
                                     G_APPLICATION_DEFAULT_FLAGS, nullptr));
}
