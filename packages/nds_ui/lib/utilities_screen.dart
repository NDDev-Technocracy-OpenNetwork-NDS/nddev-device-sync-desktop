import 'dart:async';

import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:nddev_opennetwork_design_system/opennetwork_design_system.dart';
import 'package:nds_native/nds_native.dart' as native;
import 'package:nds_native/clipboard.dart' as archive;

import 'l10n/app_localizations.dart';
import 'module_settings.dart';
import 'activity.dart';

class UtilitiesScreen extends StatefulWidget {
  const UtilitiesScreen({super.key, required this.client});
  final native.NdsClient client;
  @override
  State<UtilitiesScreen> createState() => _UtilitiesState();
}

class _UtilitiesState extends State<UtilitiesScreen> {
  final _text = TextEditingController();
  Timer? _clock;
  native.ClipboardView? _clipboard;
  native.LocalAgent? _agent;
  native.LocalAgent? _retiringAgent;
  Future<bool>? _agentShutdown;
  bool _shutdownFailed = false;
  bool get _cleanupHeld => _retiringAgent != null && _shutdownFailed;
  Future<void>? _transition;
  Future<void>? _operation;
  int _epoch = 0;
  String? _scope;
  bool _leaving = false, _allowPop = false;
  archive.Page? _archive;
  native.IncidentPage? _incidents;
  String? _error;
  bool _copied = false;
  bool _busy = false, _polling = false;
  bool get _paused => !ApplicationActivityScope.read(context).value;
  Completer<void>? _pollCompletion;
  bool _clipboardUnavailable = false, _incidentsUnavailable = false;
  @override
  void initState() {
    super.initState();
    _scope = _currentScope();
    _clock = Timer.periodic(const Duration(seconds: 2), (_) {
      if (mounted &&
          !_busy &&
          !_paused &&
          ModalRoute.of(context)?.isCurrent == true) {
        unawaited(_poll());
      }
    });
    unawaited(_poll());
  }

  @override
  void dispose() {
    _epoch++;
    _clock?.cancel();
    _text.clear();
    _text.dispose();
    _agent?.close();
    _retiringAgent?.close();
    // Cancellation-only fallback. Retained transitions keep already-started
    // discovery/shutdown and its exact opaque alive until positive completion.
    super.dispose();
  }

  String? _currentScope() {
    try {
      final view = widget.client.snapshot();
      return '${view.authenticated}:${view.tenantId}:${view.userId}:${view.currentDeviceId}';
    } catch (error) {
      if (error is native.ClientError) error.dispose();
      return null;
    }
  }

  bool _current(native.NdsClient client, int epoch) =>
      mounted &&
      epoch == _epoch &&
      identical(client, widget.client) &&
      _scope != null &&
      _scope == _currentScope();

  @override
  void didUpdateWidget(covariant UtilitiesScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(oldWidget.client, widget.client) ||
        _scope != _currentScope()) {
      _ownerChanged();
    }
  }

  void _ownerChanged() {
    _epoch++;
    _scope = _currentScope();
    _agent?.close();
    _archive = null;
    _incidents = null;
    _clipboard = null;
    _text.clear();
    _busy = true;
    _transitionOwner();
  }

  Future<bool> _retireAgent(native.LocalAgent? agent) {
    if (_retiringAgent != null) return _agentShutdown!;
    if (agent == null) return Future.value(true);
    _retiringAgent = agent;
    agent.close();
    _agentShutdown = _shutdownAgent(agent);
    return _agentShutdown!;
  }

  Future<bool> _shutdownAgent(native.LocalAgent agent) async {
    try {
      await agent.shutdown();
      if (identical(_agent, agent)) _agent = null;
      agent.dispose();
      _retiringAgent = null;
      return true;
    } catch (error) {
      var code = 'unexpected';
      if (error is native.ClientError) {
        code = error.code();
        error.dispose();
      }
      _shutdownFailed = true;
      if (mounted) {
        setState(() {
          _error = code;
          _busy = true;
        });
      }
      return false;
    }
  }

  Future<void> _transitionOwner() {
    if (_transition case final transition?) return transition;
    late final Future<void> owned;
    owned = _finishTransition().whenComplete(() {
      if (identical(_transition, owned)) {
        _transition = null;
        if (mounted && _retiringAgent == null && !_leaving) {
          setState(() => _busy = false);
        }
      }
    });
    _transition = owned;
    return owned;
  }

  Future<void> _finishTransition() async {
    _agent?.close();
    // A discovery finishing in this action retires its own stale result before
    // the action completes. Do not dispose an opaque its continuation can use.
    await _operation;
    await _pollCompletion?.future;
    if (!await _retireAgent(_agent)) return;
    if (!mounted) return;
    if (_leaving) {
      setState(() => _allowPop = true);
      Navigator.of(context).pop();
    } else {
      setState(() => _busy = false);
    }
  }

  void _leave() {
    if (_leaving || _retiringAgent != null) return;
    _leaving = true;
    _epoch++;
    _agent?.close();
    setState(() => _busy = true);
    _transitionOwner();
  }

  Future<void> _action(Future<void> Function() work) {
    if (_busy || _retiringAgent != null || _transition != null || _leaving) {
      return Future.value();
    }
    late final Future<void> owned;
    owned = _actionInner(work).whenComplete(() {
      if (identical(_operation, owned)) _operation = null;
    });
    _operation = owned;
    return owned;
  }

  Future<void> _actionInner(Future<void> Function() work) async {
    final client = widget.client;
    final epoch = _epoch;
    setState(() {
      _busy = true;
      _copied = false;
      _error = null;
    });
    try {
      // A user action owns the next local read/use. Finish the current bounded
      // background read instead of racing the same vault admission gate.
      await _pollCompletion?.future;
      if (!_current(client, epoch)) return;
      await work();
    } catch (error) {
      var code = 'unexpected';
      if (error is native.ClientError) {
        code = error.code();
        error.dispose();
      } else if (error is native.VaultError) {
        code = error.code();
      } else if (error is native.ModuleError) {
        code = error.name;
      }
      if (_current(client, epoch)) setState(() => _error = code);
    } finally {
      if (mounted) {
        setState(
          () =>
              _busy = _transition != null || _retiringAgent != null || _leaving,
        );
      }
    }
  }

  Future<void> _poll() async {
    if (!mounted) return;
    if (_currentScope() == null) {
      setState(() {
        _busy = true;
        _error = 'unexpected';
      });
      return;
    }
    if (_scope != _currentScope()) {
      _ownerChanged();
      return;
    }
    if (_polling || _transition != null || _retiringAgent != null || _leaving) {
      return;
    }
    final client = widget.client;
    final epoch = _epoch;
    _polling = true;
    final completion = Completer<void>();
    _pollCompletion = completion;
    try {
      final value = await client.clipboardSnapshot();
      if (_current(client, epoch)) {
        setState(() {
          _clipboard = value;
          _clipboardUnavailable = false;
        });
      }
    } catch (_) {
      if (_current(client, epoch)) {
        setState(() {
          _clipboardUnavailable = true;
          _clipboard = null;
          if (!client.snapshot().authenticated) {
            _archive = null;
            _incidents = null;
            _text.clear();
          }
        });
      }
    } finally {
      _polling = false;
      _pollCompletion = null;
      completion.complete();
    }
  }

  Future<void> _alerts({String after = ''}) async {
    final client = widget.client;
    final epoch = _epoch;
    try {
      final page = await client.incidents(after: after);
      if (_current(client, epoch)) {
        setState(() {
          _incidents = page;
          _incidentsUnavailable = false;
        });
      }
    } catch (error) {
      if (error is native.ClientError) error.dispose();
      if (_current(client, epoch)) setState(() => _incidentsUnavailable = true);
    }
  }

  Future<void> _incidentAction(
    native.IncidentView incident,
    int seconds,
  ) async {
    final client = widget.client;
    final epoch = _epoch;
    final updated = await client.incidentAction(
      id: incident.id,
      revision: incident.revision,
      silenceSeconds: seconds,
    );
    if (_current(client, epoch)) {
      setState(() {
        final items = _incidents?.incidents;
        if (items != null) {
          final index = items.indexWhere((value) => value.id == updated.id);
          if (index >= 0) items[index] = updated;
        }
      });
    }
  }

  String _date(String value) {
    final parsed = DateTime.tryParse(value);
    return parsed == null
        ? value
        : DateFormat.yMd(Localizations.localeOf(context).languageCode)
              .add_Hms()
              .format(parsed.toLocal());
  }

  String _message(AppLocalizations l) => switch (_error) {
    'vault_locked' || 'authentication_failed' => l.authError,
    'device_revoked' => l.deviceRevokedHelp,
    'vault_busy' || 'clipboard_busy' || 'busy' || 'conflict' => l.operationBusy,
    'vault_cancelled' => l.moduleCancelled,
    'vault_invalid_input' || 'invalid_input' => l.vaultInputError,
    'sync_capacity_exhausted' => l.serverCapacityExhausted,
    'vault_dependency_unavailable' ||
    'transport_unavailable' ||
    'request_timeout' => l.networkError,
    'clipboard_private' ||
    'clipboard_unsupported' ||
    'clipboard_too_large' => l.clipboardRejected,
    'clipboard_unavailable' => l.clipboardUnavailable,
    'telemetry_unavailable' => l.telemetryUnavailable,
    'module_disabled' ||
    'module_permission_required' ||
    'module_configuration_conflict' => moduleFailureLabel(l, _error!),
    _ => l.genericError,
  };

  Future<void> _archiveEntries() async {
    if (!native.supportsLocalAgent()) return;
    final client = widget.client;
    final epoch = _epoch;
    var agent = _agent;
    if (agent == null) {
      agent = await client.discoverLocalAgent();
      if (!_current(client, epoch)) {
        await _retireAgent(agent);
        return;
      }
      _agent = agent;
    }
    final page = await agent.clipboardEntries(client: client);
    if (_current(client, epoch) && identical(agent, _agent)) {
      setState(() => _archive = page);
    } else {
      await _retireAgent(agent);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context)!;
    final delivery = widget.client.deliverySnapshot();
    final screen = DefaultTabController(
      length: 2,
      child: Scaffold(
        appBar: AppBar(
          title: Text(l.utilitiesTitle),
          bottom: TabBar(
            tabs: [
              Tab(text: l.sharedClipboard),
              Tab(text: l.alertsTitle),
            ],
          ),
        ),
        body: TabBarView(
          children: [
            ListView(
              padding: const EdgeInsets.all(OpenNetworkSpacing.four),
              children: [
                Text(l.clipboardHelp),
                const SizedBox(height: OpenNetworkSpacing.four),
                TextField(
                  key: const Key('shared-text'),
                  controller: _text,
                  maxLength: 8192,
                  minLines: 2,
                  maxLines: 5,
                  decoration: InputDecoration(labelText: l.sharedText),
                ),
                FilledButton(
                  key: const Key('share-text'),
                  onPressed: _busy
                      ? null
                      : () => _action(() async {
                          await widget.client.clipboardPublishText(
                            text: _text.text,
                          );
                          if (mounted) _text.clear();
                          await _poll();
                        }),
                  child: Text(l.shareText),
                ),
                if (native.supportsNativeClipboardRead())
                  OutlinedButton(
                    key: const Key('share-native-clipboard'),
                    onPressed: _busy
                        ? null
                        : () => _action(() async {
                            await widget.client.clipboardPublishNative();
                            await _poll();
                          }),
                    child: Text(l.shareNativeClipboard),
                  ),
                if (native.supportsLocalAgent())
                  TextButton(
                    onPressed: _busy ? null : () => _action(_archiveEntries),
                    child: Text(l.clipboardArchive),
                  ),
                for (final entry in _archive?.items ?? <archive.Entry>[])
                  if (entry.mimes.any(
                    (mime) =>
                        mime == 'text/plain' ||
                        mime.toLowerCase() == 'text/plain;charset=utf-8',
                  ))
                    ListTile(
                      title: Text(
                        entry.preview ?? l.sharedText,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                      ),
                      trailing: TextButton(
                        onPressed: _busy
                            ? null
                            : () => _action(() async {
                                await widget.client.clipboardPublishArchive(
                                  agent: _agent!,
                                  entry: entry.id,
                                  mime: entry.mimes.firstWhere(
                                    (mime) =>
                                        mime == 'text/plain' ||
                                        mime.toLowerCase() ==
                                            'text/plain;charset=utf-8',
                                  ),
                                );
                                await _poll();
                              }),
                        child: Text(l.shareText),
                      ),
                    ),
                const Divider(),
                if (_clipboardUnavailable) Text(l.clipboardUnavailable),
                if (_copied) Text(l.clipboardCopied),
                Text(
                  _clipboard?.present == true
                      ? l.clipboardReady(
                          _clipboard!.serverSeq.toInt(),
                          _clipboard!.bytes,
                        )
                      : l.clipboardEmpty,
                ),
                FilledButton(
                  key: const Key('copy-shared-clipboard'),
                  onPressed:
                      _busy ||
                          !native.supportsNativeClipboard() ||
                          _clipboard?.present != true
                      ? null
                      : () {
                          String? ticket;
                          try {
                            ticket = widget.client.reserveClipboardCopy();
                          } catch (_) {
                            setState(
                              () => _error = 'vault_dependency_unavailable',
                            );
                            return;
                          }
                          _action(() async {
                            await widget.client.clipboardCopy(ticket: ticket);
                            if (mounted) setState(() => _copied = true);
                          });
                        },
                  child: Text(l.copySharedClipboard),
                ),
                if (_busy && !_cleanupHeld) const LinearProgressIndicator(),
                if (_error != null)
                  Text(
                    _cleanupHeld ? l.moduleShutdownUnconfirmed : _message(l),
                    key: const Key('utility-error'),
                  ),
              ],
            ),
            ListView(
              padding: const EdgeInsets.all(OpenNetworkSpacing.four),
              children: [
                if (delivery.localPersistenceReady != null)
                  Text(
                    delivery.localPersistenceReady!
                        ? l.telemetryPersistenceReady
                        : l.telemetryPersistenceUnavailable,
                  ),
                Text(l.eventsLabel),
                Text(
                  l.telemetryDelivery(
                    delivery.pending,
                    delivery.delivered.toInt(),
                    delivery.dropped.toInt(),
                    delivery.failures.toInt(),
                  ),
                ),
                Text(l.spansLabel),
                Text(
                  l.telemetryDelivery(
                    delivery.spansPending,
                    delivery.spansDelivered.toInt(),
                    delivery.spansDropped.toInt(),
                    delivery.spansFailures.toInt(),
                  ),
                ),
                Text(switch (delivery.policy) {
                  native.ExportPolicy.unknown => l.exportUnknown,
                  native.ExportPolicy.enabled => l.exportEnabled,
                  native.ExportPolicy.disabled => l.exportDisabled,
                }),
                if (delivery.paused) Text(l.exportPaused),
                if (delivery.lastSuccessAt != BigInt.zero)
                  Text(
                    l.exportLastSeen(
                      _date(
                        DateTime.fromMillisecondsSinceEpoch(
                          delivery.lastSuccessAt.toInt() * 1000,
                          isUtc: true,
                        ).toIso8601String(),
                      ),
                    ),
                  ),
                Text(
                  delivery.initialized
                      ? l.telemetryInitialized
                      : l.telemetryUnavailable,
                ),
                TextButton(
                  key: const Key('refresh-incidents'),
                  onPressed: _busy ? null : () => _action(_alerts),
                  child: Text(l.moduleRefresh),
                ),
                if (_incidentsUnavailable) Text(l.incidentsUnavailable),
                if (_incidents != null)
                  Text(l.incidentsObserved(_date(_incidents!.generatedAt))),
                if (_incidents?.incidents.isEmpty == true) Text(l.noIncidents),
                for (final incident
                    in _incidents?.incidents ?? <native.IncidentView>[])
                  Card(
                    child: Padding(
                      padding: const EdgeInsets.all(OpenNetworkSpacing.four),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            incident.service,
                            style: Theme.of(context).textTheme.titleMedium,
                          ),
                          Text(
                            l.incidentState(
                              incident.kind,
                              incident.state,
                              incident.occurrences.toInt(),
                            ),
                          ),
                          if (incident.silencedUntil.isNotEmpty)
                            Text(
                              '${l.silenceIncident} · ${l.expires}: ${_date(incident.silencedUntil)}',
                            ),
                          if (incident.state != 'resolved')
                            Wrap(
                              spacing: OpenNetworkSpacing.four,
                              children: [
                                TextButton(
                                  onPressed: _busy
                                      ? null
                                      : () => _action(() async {
                                          await _incidentAction(incident, 0);
                                        }),
                                  child: Text(l.acknowledgeIncident),
                                ),
                                TextButton(
                                  onPressed: _busy
                                      ? null
                                      : () => _action(() async {
                                          await _incidentAction(incident, 3600);
                                        }),
                                  child: Text(l.silenceIncident),
                                ),
                              ],
                            ),
                        ],
                      ),
                    ),
                  ),
                if (_incidents?.nextCursor.isNotEmpty == true)
                  TextButton(
                    onPressed: _busy
                        ? null
                        : () => _action(() async {
                            await _alerts(after: _incidents!.nextCursor);
                          }),
                    child: Text(l.moreIncidents),
                  ),
                if (_busy && !_cleanupHeld) const LinearProgressIndicator(),
                if (_error != null)
                  Text(
                    _cleanupHeld ? l.moduleShutdownUnconfirmed : _message(l),
                    key: const Key('utility-error'),
                  ),
              ],
            ),
          ],
        ),
      ),
    );
    return PopScope<void>(
      canPop:
          _allowPop ||
          (_agent == null &&
              _retiringAgent == null &&
              _operation == null &&
              _transition == null),
      onPopInvokedWithResult: (didPop, _) {
        if (!didPop) _leave();
      },
      child: screen,
    );
  }
}
