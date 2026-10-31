import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:nds_native/nds_native.dart' as native;

import 'l10n/app_localizations.dart';

const devicePurposes = [
  'server-hosting',
  'server-dev-desktop',
  'server-desktop',
  'build-dev-desktop',
  'dev-desktop',
  'desktop',
];

String devicePurposeLabel(AppLocalizations l, String value) => switch (value) {
  'server-hosting' => l.purposeServerHosting,
  'server-dev-desktop' => l.purposeServerDevDesktop,
  'server-desktop' => l.purposeServerDesktop,
  'build-dev-desktop' => l.purposeBuildDevDesktop,
  'dev-desktop' => l.purposeDevDesktop,
  'desktop' => l.purposeDesktop,
  _ => value,
};

String deviceControlError(AppLocalizations l, String code) => switch (code) {
  'recent_authentication_required' => l.controlRecentSignIn,
  'operation_conflict' => l.controlChangedElsewhere,
  'device_policy_unavailable' => l.controlUnavailable,
  _ => l.providerFailed,
};

/// Presentation of the authenticated device control policy (ADR 0018). Rust
/// owns validation, compare-and-set and admission; this is only a draft.
class DeviceControlCard extends StatefulWidget {
  const DeviceControlCard({
    super.key,
    required this.client,
    required this.onChanged,
  });
  final native.NdsClient client;
  final Future<void> Function() onChanged;
  @override
  State<DeviceControlCard> createState() => _DeviceControlCardState();
}

class _DeviceControlCardState extends State<DeviceControlCard> {
  native.DeviceControlView? _current;
  String _mode = 'manual';
  Set<String> _purposes = {};
  bool _busy = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void didUpdateWidget(covariant DeviceControlCard oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(oldWidget.client, widget.client)) _load();
  }

  void _adopt(native.DeviceControlView value) {
    _current = value;
    _mode = value.mode;
    _purposes = value.purposes.toSet();
  }

  Future<String> _code(Object error) async {
    if (error is native.ClientError) {
      final code = error.code();
      error.dispose();
      return code;
    }
    return 'unexpected';
  }

  Future<void> _load() async {
    final client = widget.client;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final value = await client.deviceControl();
      if (mounted && identical(client, widget.client)) {
        setState(() => _adopt(value));
      }
    } catch (error) {
      final code = await _code(error);
      if (mounted && identical(client, widget.client)) {
        setState(() => _error = code);
      }
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  bool get _changed {
    final current = _current;
    return current != null &&
        (_mode != current.mode ||
            !setEquals(_purposes, current.purposes.toSet()));
  }

  Future<void> _save() async {
    final client = widget.client;
    final current = _current;
    if (_busy || current == null || current.revision == BigInt.zero) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final value = await client.setDeviceControl(
        expectedRevision: current.revision,
        mode: _mode,
        purposes: _purposes.toList()..sort(),
      );
      if (mounted && identical(client, widget.client)) {
        setState(() => _adopt(value));
      }
      await widget.onChanged();
    } catch (error) {
      final code = await _code(error);
      if (mounted && identical(client, widget.client)) {
        setState(() => _error = code);
      }
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context)!;
    final current = _current;
    final verified = current != null && current.revision > BigInt.zero;
    return Column(
      key: const Key('device-control'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(l.deviceControl, style: Theme.of(context).textTheme.titleMedium),
        if (current != null)
          Text(switch (current.source) {
            'server' => l.controlFromServer(current.revision.toString()),
            'cached' => l.controlCached(current.revision.toString()),
            _ => l.controlDefaultManual,
          }),
        SegmentedButton<String>(
          key: const Key('device-control-mode'),
          segments: [
            ButtonSegment(value: 'fullauto', label: Text(l.controlFullauto)),
            ButtonSegment(value: 'manual', label: Text(l.controlManual)),
          ],
          selected: {_mode},
          onSelectionChanged: _busy || !verified
              ? null
              : (value) => setState(() => _mode = value.single),
        ),
        Text(_mode == 'fullauto' ? l.controlFullautoHint : l.controlManualHint),
        Wrap(
          spacing: 8,
          children: [
            for (final purpose in devicePurposes)
              FilterChip(
                key: Key('device-purpose-$purpose'),
                label: Text(devicePurposeLabel(l, purpose)),
                selected: _purposes.contains(purpose),
                onSelected: _busy || !verified
                    ? null
                    : (selected) => setState(() {
                        if (selected) {
                          _purposes.add(purpose);
                        } else if (_purposes.length > 1) {
                          _purposes.remove(purpose);
                        }
                      }),
              ),
          ],
        ),
        Row(
          children: [
            FilledButton(
              key: const Key('device-control-save'),
              onPressed: _busy || !_changed ? null : _save,
              child: Text(l.controlSave),
            ),
            const SizedBox(width: 8),
            TextButton(
              key: const Key('device-control-reload'),
              onPressed: _busy ? null : _load,
              child: Text(l.moduleRefresh),
            ),
          ],
        ),
        if (_error case final code?)
          Text(
            deviceControlError(l, code),
            key: const Key('device-control-error'),
            style: TextStyle(color: Theme.of(context).colorScheme.error),
          ),
      ],
    );
  }
}
