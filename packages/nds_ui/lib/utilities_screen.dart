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
    _clock?.cancel();
    _text.clear();
    _text.dispose();
    _agent?.close();
    _agent?.dispose();
    super.dispose();
  }

  Future<void> _action(Future<void> Function() work) async {
    if (_busy) return;
    setState(() {
      _busy = true;
      _copied = false;
      _error = null;
    });
    try {
      // A user action owns the next local read/use. Finish the current bounded
      // background read instead of racing the same vault admission gate.
      await _pollCompletion?.future;
      if (!mounted) return;
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
      if (mounted) setState(() => _error = code);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _poll() async {
    if (_polling) return;
    _polling = true;
    final completion = Completer<void>();
    _pollCompletion = completion;
    try {
      final value = await widget.client.clipboardSnapshot();
      if (mounted) {
        setState(() {
          _clipboard = value;
          _clipboardUnavailable = false;
        });
      }
    } catch (_) {
      if (mounted) {
        setState(() {
          _clipboardUnavailable = true;
          _clipboard = null;
          if (!widget.client.snapshot().authenticated) {
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
    try {
      final page = await widget.client.incidents(after: after);
      if (mounted) {
        setState(() {
          _incidents = page;
          _incidentsUnavailable = false;
        });
      }
    } catch (error) {
      if (error is native.ClientError) error.dispose();
      if (mounted) setState(() => _incidentsUnavailable = true);
    }
  }

  Future<void> _incidentAction(
    native.IncidentView incident,
    int seconds,
  ) async {
    final updated = await widget.client.incidentAction(
      id: incident.id,
      revision: incident.revision,
      silenceSeconds: seconds,
    );
    if (mounted) {
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
    _agent ??= await widget.client.discoverLocalAgent();
    final page = await _agent!.clipboardEntries(client: widget.client);
    if (mounted) setState(() => _archive = page);
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context)!;
    final delivery = widget.client.deliverySnapshot();
    return DefaultTabController(
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
                if (_busy) const LinearProgressIndicator(),
                if (_error != null)
                  Text(_message(l), key: const Key('utility-error')),
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
                if (_busy) const LinearProgressIndicator(),
                if (_error != null)
                  Text(_message(l), key: const Key('utility-error')),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
