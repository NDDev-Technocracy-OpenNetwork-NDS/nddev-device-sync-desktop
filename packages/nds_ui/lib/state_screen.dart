import 'dart:async';

import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:nddev_opennetwork_design_system/opennetwork_design_system.dart';
import 'package:nds_native/nds_native.dart' as native;

import 'l10n/app_localizations.dart';
import 'activity.dart';
import 'module_settings.dart';

class StateScreen extends StatefulWidget {
  const StateScreen({super.key, required this.client});
  final native.NdsClient client;
  @override
  State<StateScreen> createState() => _StateScreenState();
}

class _StateScreenState extends State<StateScreen> {
  native.StatePage? _page;
  bool _busy = false;
  String? _error;
  Timer? _clock;
  bool get _paused => !ApplicationActivityScope.read(context).value;
  String _after = '';
  BigInt? _cursor;
  BigInt? _localRevision;
  @override
  void initState() {
    super.initState();
    _clock = Timer.periodic(const Duration(seconds: 1), (_) {
      if (mounted &&
          !_busy &&
          !_paused &&
          ModalRoute.of(context)?.isCurrent == true) {
        unawaited(_read(after: _after, polling: true));
      }
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _read();
    });
  }

  @override
  void dispose() {
    _clock?.cancel();
    super.dispose();
  }

  Future<void> _read({
    String after = '',
    bool publish = false,
    bool polling = false,
  }) async {
    if (_busy) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      if (publish) {
        await widget.client.publishDeviceState();
      }
      final view = await widget.client.vaultSnapshot();
      if (!view.unlocked) {
        if (mounted) {
          setState(() {
            _page = null;
            _cursor = null;
            _localRevision = null;
            _error = 'vault_locked';
          });
        }
        return;
      }
      if (view.syncError == 'sync_capacity_exhausted' && mounted) {
        setState(() => _error = view.syncError);
      }
      if (polling &&
          _cursor == view.cursor &&
          _localRevision == view.localRevision) {
        return;
      }
      final page = await widget.client.syncStates(after: after);
      if (mounted) {
        setState(() {
          _page = page;
          _cursor = page.observedSeq;
          _localRevision = page.localRevision;
          _after = after;
        });
      }
    } catch (error) {
      final code = error is native.VaultError ? error.code() : 'unexpected';
      if (mounted && (!polling || code != 'vault_busy')) {
        setState(() => _error = code);
      }
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  String _date(BigInt value) => value == BigInt.zero
      ? AppLocalizations.of(context)!.notChecked
      : DateFormat.yMd(Localizations.localeOf(context).toString())
            .add_Hms()
            .format(DateTime.fromMillisecondsSinceEpoch(value.toInt()));
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context)!;
    return Scaffold(
      appBar: AppBar(title: Text(l.syncedState)),
      body: SafeArea(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(OpenNetworkSpacing.six),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(l.stateHelp),
              OutlinedButton(
                key: const Key('publish-device-state'),
                onPressed: _busy ? null : () => _read(publish: true),
                child: Text(l.publishDeviceState),
              ),
              TextButton(
                key: const Key('state-refresh'),
                onPressed: _busy ? null : () => _read(),
                child: Text(l.moduleRefresh),
              ),
              if (_page != null) ...[
                if (_page!.items.isEmpty) Text(l.noRecords),
                for (final item in _page!.items)
                  Card(
                    key: ValueKey('state-${item.id}'),
                    child: Padding(
                      padding: const EdgeInsets.all(OpenNetworkSpacing.four),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(switch (item.kind) {
                            'device_state' => l.stateDevices,
                            'module_state' ||
                            'module_configuration' => l.module,
                            _ => l.sessionTitle,
                          }),
                          Text(
                            item.kind == 'account_state' &&
                                    item.label == 'email_otp'
                                ? l.emailMethod
                                : item.kind == 'account_state' &&
                                      item.label == 'github'
                                ? 'GitHub'
                                : item.kind == 'account_state' &&
                                      item.label == 'passkey'
                                ? l.passkeyMethod
                                : item.label,
                          ),
                          if (item.platform.isNotEmpty) Text(item.platform),
                          if (item.version.isNotEmpty) Text(item.version),
                          Text('${l.stateDevice}: ${item.deviceId}'),
                          Text('${l.verified}: ${_date(item.observedAtMs)}'),
                          if (item.kind == 'module_state')
                            Text(
                              !item.configured
                                  ? l.providerUnavailable
                                  : switch (item.reachability) {
                                      'reachable' => l.reachable,
                                      'failed' => l.providerFailed,
                                      'unavailable' => l.providerUnavailable,
                                      _ => l.notChecked,
                                    },
                            ),
                          if (item.kind == 'module_configuration') ...[
                            Text(
                              '${l.moduleEnabled}: ${item.enabled == true ? l.checkOk : l.checkError}',
                            ),
                            for (final permission in item.permissions)
                              Text(modulePermissionLabel(l, permission)),
                          ],
                          if (item.checkedAtMs != BigInt.zero)
                            Text('${l.lastRun}: ${_date(item.checkedAtMs)}'),
                          if (item.expiresAtMs != BigInt.zero)
                            Text('${l.expires}: ${_date(item.expiresAtMs)}'),
                          Text(l.vaultRevision(item.revision.toString())),
                          if (item.pendingId != null)
                            Text(l.vaultRecordPending),
                          if (item.conflicted) Text(l.vaultConflict),
                        ],
                      ),
                    ),
                  ),
                if (_page!.hasMore)
                  OutlinedButton(
                    onPressed: _busy
                        ? null
                        : () => _read(after: _page!.nextAfter),
                    child: Text(l.nextPage),
                  ),
              ],
              if (_busy) const LinearProgressIndicator(),
              if (_error != null)
                Text(
                  _error == 'vault_locked'
                      ? l.vaultLocked
                      : _error == 'sync_capacity_exhausted'
                      ? l.serverCapacityExhausted
                      : _error == 'vault_busy'
                      ? l.operationBusy
                      : _error == 'vault_cancelled'
                      ? l.moduleCancelled
                      : l.statePublishError,
                  key: const Key('state-error'),
                ),
            ],
          ),
        ),
      ),
    );
  }
}
