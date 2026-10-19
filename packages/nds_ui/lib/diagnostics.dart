import 'dart:ui';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:nds_native/nds_native.dart' as native;

import 'l10n/app_localizations.dart';

/// Owns Flutter's process hooks; the Rust SDK owns records, policy and bounds.
/// Exception objects, stack traces and build diagnostics never cross FFI.
class ApplicationDiagnostics {
  ApplicationDiagnostics() {
    _previousFramework = FlutterError.onError;
    _previousAsync = PlatformDispatcher.instance.onError;
    _previousWidget = ErrorWidget.builder;
    _framework = (_) => reportFailure(native.FailureKind.flutterFramework);
    _async = (_, _) {
      reportFailure(native.FailureKind.dartUnhandled);
      return true;
    };
    _errorWidget = (_) =>
        ErrorWidget.withDetails(message: _messages.genericError);
    FlutterError.onError = _framework;
    PlatformDispatcher.instance.onError = _async;
    ErrorWidget.builder = _errorWidget;
    _lifecycle = AppLifecycleListener(
      onExitRequested: () async {
        if (_nativeAvailable) {
          try {
            await native.shutdownTelemetry();
          } catch (_) {
            _unavailable();
            return AppExitResponse.cancel;
          }
        }
        return AppExitResponse.exit;
      },
    );
  }

  late final FlutterExceptionHandler? _previousFramework;
  late final bool Function(Object, StackTrace)? _previousAsync;
  late final ErrorWidgetBuilder _previousWidget;
  late final FlutterExceptionHandler _framework;
  late final bool Function(Object, StackTrace) _async;
  late final ErrorWidgetBuilder _errorWidget;
  late final AppLifecycleListener _lifecycle;
  bool _nativeAvailable = false;
  bool _reporting = false;
  bool _disposed = false;
  bool _failed = false;
  bool _unavailableReported = false;
  Locale? locale;

  bool get hadFailure => _failed;
  bool get reportingUnavailable => _unavailableReported;
  AppLocalizations get _messages => lookupAppLocalizations(
    (locale ?? PlatformDispatcher.instance.locale).languageCode == 'ru'
        ? const Locale('ru')
        : const Locale('en'),
  );

  void nativeInitialized() => _nativeAvailable = true;
  void lifecycle(native.ClientLifecycle state) {
    if (!_nativeAvailable) return;
    _record(() => native.recordClientLifecycle(state: state));
  }

  void clientReady() {
    if (!_failed) {
      _record(
        () => native.recordClientLifecycle(state: native.ClientLifecycle.ready),
      );
    }
  }

  void reportFailure(native.FailureKind kind) {
    _failed = true;
    _record(() => native.recordClientFailure(kind: kind));
  }

  void _record(bool Function() operation) {
    if (_disposed || _reporting) return;
    _reporting = true;
    try {
      if (!_nativeAvailable || !operation()) _unavailable();
    } catch (_) {
      _unavailable();
    } finally {
      _reporting = false;
    }
  }

  void _unavailable() {
    if (_unavailableReported) return;
    _unavailableReported = true;
    // The SDK cannot emit when the native bridge/sink is absent. One fixed local
    // fallback, also reflected in UI, does not pretend to be a delivered event.
    debugPrint('NDS diagnostics unavailable: native_bridge_or_sink');
  }

  void dispose() {
    if (_disposed) return;
    _disposed = true;
    _lifecycle.dispose();
    if (FlutterError.onError == _framework) {
      FlutterError.onError = _previousFramework;
    }
    if (PlatformDispatcher.instance.onError == _async) {
      PlatformDispatcher.instance.onError = _previousAsync;
    }
    if (ErrorWidget.builder == _errorWidget) {
      ErrorWidget.builder = _previousWidget;
    }
  }
}
