import 'dart:async';

import 'package:flutter/material.dart';
import 'package:nddev_opennetwork_design_system/opennetwork_design_system.dart';
import 'package:nds_native/src/rust/api/app_update.dart' as native;

import 'l10n/app_localizations.dart';

class AppUpdateScreen extends StatefulWidget {
  const AppUpdateScreen({super.key});

  @override
  State<AppUpdateScreen> createState() => _AppUpdateState();
}

class _AppUpdateState extends State<AppUpdateScreen> {
  late final native.AppUpdates _updates;
  native.AppUpdateView? _view;
  Timer? _progress;
  bool _busy = false;
  native.AppUpdateError? _failure;
  bool _bridgeFailure = false;

  @override
  void initState() {
    super.initState();
    _updates = native.AppUpdates();
    _view = _updates.snapshot();
    unawaited(_run(_updates.restore));
  }

  Future<void> _run(Future<native.AppUpdateView> Function() operation) async {
    if (_busy) return;
    setState(() {
      _busy = true;
      _failure = null;
      _bridgeFailure = false;
    });
    _progress = Timer.periodic(const Duration(milliseconds: 250), (_) {
      if (mounted) setState(() => _view = _updates.snapshot());
    });
    try {
      await operation();
    } catch (error) {
      // Rust owns stable errors. Exception bodies/URLs/paths never enter UI.
      if (error is native.AppUpdateError) {
        _failure = error;
      } else {
        _bridgeFailure = true;
      }
    } finally {
      _progress?.cancel();
      if (mounted) {
        setState(() {
          _view = _updates.snapshot();
          _busy = false;
        });
      }
    }
  }

  @override
  void dispose() {
    _progress?.cancel();
    _updates.cancel();
    unawaited(_updates.close());
    super.dispose();
  }

  String _message(AppLocalizations l, native.AppUpdateView view) {
    if (_bridgeFailure) return l.appUpdateInvalid;
    if ((_failure ?? view.error) case final error?) {
      return switch (error) {
        native.AppUpdateError.unsupported => l.appUpdateUnsupported,
        native.AppUpdateError.notConfigured => l.appUpdateNotConfigured,
        native.AppUpdateError.busy => l.appUpdateBusy,
        native.AppUpdateError.cancelled => l.appUpdateCancelled,
        native.AppUpdateError.timeout => l.appUpdateTimeout,
        native.AppUpdateError.network => l.appUpdateNetwork,
        native.AppUpdateError.tooLarge => l.appUpdateTooLarge,
        native.AppUpdateError.signature => l.appUpdateSignature,
        native.AppUpdateError.manifest => l.appUpdateInvalid,
        native.AppUpdateError.source => l.appUpdateSource,
        native.AppUpdateError.compatibility => l.appUpdateCompatibility,
        native.AppUpdateError.notNewer => l.appUpdateNotNewer,
        native.AppUpdateError.storage => l.appUpdateStorage,
      };
    }
    return switch (view.status) {
      native.AppUpdateStatus.unsupported => l.appUpdateUnsupported,
      native.AppUpdateStatus.notConfigured => l.appUpdateNotConfigured,
      native.AppUpdateStatus.idle => l.appUpdateIdle,
      native.AppUpdateStatus.checking => l.appUpdateChecking,
      native.AppUpdateStatus.available => l.appUpdateAvailable,
      native.AppUpdateStatus.downloading => l.appUpdateDownloading,
      native.AppUpdateStatus.staged => l.appUpdateStaged,
      native.AppUpdateStatus.failed => l.appUpdateInvalid,
    };
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context)!;
    final view = _view;
    return Scaffold(
      appBar: AppBar(title: Text(l.appUpdateTitle)),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(OpenNetworkSpacing.six),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (view != null) ...[
              Text(_message(l, view), key: const Key('app-update-status')),
              if (view.version.isNotEmpty)
                Text(l.appUpdateVersion(view.version)),
              if (view.status == native.AppUpdateStatus.downloading) ...[
                LinearProgressIndicator(
                  value: view.totalBytes > BigInt.zero
                      ? (view.downloadedBytes.toDouble() /
                                view.totalBytes.toDouble())
                            .clamp(0, 1)
                      : null,
                ),
                Text(
                  l.appUpdateProgress(
                    '${view.downloadedBytes}',
                    '${view.totalBytes}',
                  ),
                ),
              ],
              const SizedBox(height: OpenNetworkSpacing.four),
              Wrap(
                spacing: OpenNetworkSpacing.three,
                runSpacing: OpenNetworkSpacing.three,
                children: [
                  FilledButton(
                    key: const Key('app-update-check'),
                    onPressed: _busy
                        ? null
                        : () => unawaited(_run(_updates.check)),
                    child: Text(l.appUpdateCheck),
                  ),
                  if (view.status == native.AppUpdateStatus.available)
                    FilledButton(
                      key: const Key('app-update-download'),
                      onPressed: _busy
                          ? null
                          : () => unawaited(_run(_updates.stage)),
                      child: Text(l.appUpdateDownload),
                    ),
                  if (_busy)
                    OutlinedButton(
                      key: const Key('app-update-cancel'),
                      onPressed: _updates.cancel,
                      child: Text(l.appUpdateCancel),
                    ),
                ],
              ),
            ],
          ],
        ),
      ),
    );
  }
}
