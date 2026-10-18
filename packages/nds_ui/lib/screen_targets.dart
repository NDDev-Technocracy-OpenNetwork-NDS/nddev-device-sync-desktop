import 'dart:async';

import 'package:flutter/material.dart';
import 'package:nddev_opennetwork_design_system/opennetwork_design_system.dart';
import 'package:nds_native/nds_native.dart' as native;

import 'activity.dart';
import 'l10n/app_localizations.dart';
import 'screen_viewer.dart';

/// Transient presentation of server-authorized discovery; no device key or
/// permission is retained here. ScreenViewer obtains fresh authority again.
class ScreenTargets extends StatefulWidget {
  const ScreenTargets({super.key, required this.client, required this.userId});
  final native.NdsClient client;
  final String userId;
  @override
  State<ScreenTargets> createState() => _ScreenTargetsState();
}

class _ScreenTargetsState extends State<ScreenTargets> {
  List<native.ScreenTargetView>? _targets;
  bool _busy = false;
  bool _failed = false;
  late final String _scope;
  Timer? _watch;
  ApplicationActivity? _activity;
  String get _currentScope {
    final view = widget.client.snapshot();
    return view.authenticated
        ? '${view.tenantId}/${view.userId}/${view.currentDeviceId}'
        : '';
  }

  @override
  void initState() {
    super.initState();
    _scope = _currentScope;
    _watch = Timer.periodic(
      const Duration(seconds: 5),
      (_) => unawaited(_load()),
    );
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final activity = ApplicationActivityScope.of(context);
    if (_activity != activity) {
      _activity?.removeListener(_changed);
      _activity = activity;
      activity.addListener(_changed);
      WidgetsBinding.instance.addPostFrameCallback((_) => _changed());
    }
  }

  void _changed() {
    if (mounted && _activity?.value == true) unawaited(_load());
  }

  Future<void> _load() async {
    if (!mounted) return;
    if (_currentScope != _scope) {
      setState(() => _targets = null);
      if (ModalRoute.of(context)?.isCurrent == true) {
        Navigator.of(context).pop();
      }
      return;
    }
    if (_busy ||
        _activity?.value != true ||
        ModalRoute.of(context)?.isCurrent != true) {
      return;
    }
    setState(() => _busy = true);
    try {
      final targets = await widget.client.screenTargets(user: widget.userId);
      if (mounted && _currentScope == _scope && _activity?.value == true) {
        setState(() {
          _targets = targets;
          _failed = false;
        });
      }
    } catch (error) {
      var busy = false;
      if (error is native.ClientError) {
        busy = error.code() == 'busy';
        error.dispose();
      }
      // Local admission did not observe a server refusal; a later owned tick
      // retries the read. All actual failures discard the administrative page.
      if (mounted && _currentScope == _scope && !busy) {
        setState(() {
          _targets = null;
          _failed = true;
        });
      }
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _view(native.ScreenTargetView target) async {
    if (_busy || _activity?.value != true || _currentScope != _scope) return;
    setState(() => _busy = true);
    try {
      await Navigator.of(context).push<void>(
        MaterialPageRoute<void>(
          builder: (_) =>
              ScreenViewer(client: widget.client, deviceId: target.deviceId),
        ),
      );
    } finally {
      if (mounted) setState(() => _busy = false);
    }
    if (mounted) await _load();
  }

  @override
  void dispose() {
    _watch?.cancel();
    _activity?.removeListener(_changed);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context)!;
    final enabled =
        !_busy && _activity?.value == true && _currentScope == _scope;
    return Scaffold(
      key: const Key('screen-targets'),
      appBar: AppBar(title: Text(l.screenTargets)),
      body: SafeArea(
        child: ListView(
          padding: const EdgeInsets.all(OpenNetworkSpacing.four),
          children: [
            if (_failed)
              Text(
                l.screenTargetsFailed,
                key: const Key('screen-targets-error'),
              ),
            if (!_failed && _targets == null)
              const Center(child: CircularProgressIndicator()),
            if (_targets?.isEmpty == true) Text(l.screenTargetsEmpty),
            for (final target in _targets ?? <native.ScreenTargetView>[])
              Card(
                child: ListTile(
                  title: Text(target.displayName),
                  subtitle: Text(
                    '${target.platform}${target.isCurrent ? ' · ${l.thisDevice}' : ''}${target.registered ? '' : ' · ${l.screenNotRegistered}'}',
                  ),
                  trailing: TextButton(
                    onPressed: enabled && target.registered && !target.isCurrent
                        ? () => unawaited(_view(target))
                        : null,
                    child: Text(l.screenView),
                  ),
                ),
              ),
            TextButton(
              onPressed: enabled ? () => unawaited(_load()) : null,
              child: Text(l.refresh),
            ),
          ],
        ),
      ),
    );
  }
}
