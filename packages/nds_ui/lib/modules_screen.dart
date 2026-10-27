import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:nddev_opennetwork_design_system/opennetwork_design_system.dart';
import 'package:nds_native/nds_native.dart' as native;
import 'package:nds_native/sysinfo.dart' as sysinfo;
import 'package:nds_native/clipboard.dart' as clipboard;
import 'package:nds_native/cleaner.dart' as cleaner;
import 'package:nds_native/updater.dart' as updater;
import 'package:nds_native/gds.dart' as gds;
import 'package:nds_native/rds.dart' as rds;

import 'l10n/app_localizations.dart';
import 'diagnostics.dart';
import 'module_settings.dart';

/// Presentation over canonical native responses. Local tools retain their state
/// and safety policies; cache binding is an explicit user action.
class ModulesScreen extends StatefulWidget {
  const ModulesScreen({
    super.key,
    required this.onLocale,
    this.diagnostics,
    this.client,
  });
  final ValueChanged<Locale> onLocale;
  final ApplicationDiagnostics? diagnostics;
  final native.NdsClient? client;
  @override
  State<ModulesScreen> createState() => _ModulesScreenState();
}

class _ModulesScreenState extends State<ModulesScreen> {
  native.LocalAgent? _agent;
  List<native.DeviceModuleView> _modules = [];
  String? _scope;
  bool _metadataError = false;
  Future<void>? _metadataRead;
  String _selected = 'sysinfo';
  Object? _result;
  String? _error;
  bool _busy = true;
  bool _cacheMutation = false;
  bool _cacheReadbackRequired = false;
  bool _cacheCompletionUnconfirmed = false;
  bool _publishing = false;
  bool _cancelled = false;
  int _epoch = 0;
  native.LocalAgent? _retiringAgent;
  Future<bool>? _agentShutdown;
  String? _shutdownError;
  bool get _cleanupHeld => _retiringAgent != null && _shutdownError != null;
  Future<void>? _transition;
  Future<void>? _operation;
  bool _rediscover = false;
  final _repository = TextEditingController();
  bool _published = false;

  Future<void> _publish() {
    if (_busy || _retiringAgent != null || _transition != null) {
      return Future.value();
    }
    return _operation = _publishInner();
  }

  Future<void> _publishInner() async {
    final client = widget.client;
    final agent = _agent;
    final epoch = _epoch;
    if (_busy || client == null || agent == null) return;
    setState(() {
      _busy = true;
      _publishing = true;
      _published = false;
      _error = null;
    });
    try {
      await client.publishLocalState(agent: agent);
      if (mounted && epoch == _epoch && identical(widget.client, client)) {
        setState(() => _published = true);
      }
    } catch (error) {
      if (mounted && epoch == _epoch && identical(widget.client, client)) {
        setState(() {
          _error = error == native.VaultError.capacityExhausted
              ? 'sync_capacity_exhausted'
              : 'publish';
        });
      }
    } finally {
      if (mounted && epoch == _epoch && identical(widget.client, client)) {
        setState(() {
          _publishing = false;
          _busy = false;
        });
      }
    }
  }

  @override
  void initState() {
    super.initState();
    _discover();
  }

  String? _currentScope() {
    try {
      final view = widget.client?.snapshot();
      return view == null
          ? null
          : '${view.authenticated}:${view.tenantId}:${view.userId}:${view.currentDeviceId}';
    } catch (error) {
      if (error is native.ClientError) error.dispose();
      return null;
    }
  }

  @override
  void didUpdateWidget(covariant ModulesScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(oldWidget.client, widget.client) ||
        _scope != _currentScope()) {
      _epoch++;
      _scope = _currentScope();
      _agent?.close();
      _modules = [];
      _result = null;
      _error = _shutdownError;
      _published = false;
      _publishing = false;
      _busy = true;
      _discover();
    } else if (!_busy && _metadataError) {
      _refreshModules();
    }
  }

  Future<void> _refreshModules() {
    if (_metadataRead case final reading?) return reading;
    late final Future<void> owned;
    owned = _readModules().whenComplete(() {
      if (identical(_metadataRead, owned)) _metadataRead = null;
    });
    _metadataRead = owned;
    return owned;
  }

  Future<void> _readModules() async {
    final client = widget.client;
    final epoch = _epoch;
    if (client == null) return;
    try {
      final modules = await client.deviceModules();
      if (mounted && epoch == _epoch && identical(client, widget.client)) {
        setState(() {
          _modules = modules;
          _metadataError = false;
        });
      }
    } catch (error) {
      if (error is native.ClientError) error.dispose();
      if (mounted && epoch == _epoch) setState(() => _metadataError = true);
    }
  }

  Future<bool> _retireAgent(native.LocalAgent? agent) {
    if (_retiringAgent != null) return _agentShutdown!;
    if (agent == null) return Future.value(true);
    _retiringAgent = agent;
    _cancelled = true;
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
      var code = 'unavailable';
      if (error is native.ClientError) {
        code = error.code();
        error.dispose();
      } else if (mounted) {
        widget.diagnostics?.reportFailure(native.FailureKind.nativeBridge);
      }
      // The exact closed owner and this completed failure stay retained. A new
      // desired client cannot replace an unresolved drain or retry it as ready.
      _shutdownError = code;
      if (mounted) {
        setState(() {
          _error = code;
          _busy = true;
        });
      }
      return false;
    }
  }

  Future<void> _discover() {
    _rediscover = true;
    if (_transition case final transition?) return transition;
    late final Future<void> owned;
    owned = _discoverCurrent().whenComplete(() {
      if (identical(_transition, owned)) _transition = null;
    });
    _transition = owned;
    return owned;
  }

  Future<void> _discoverCurrent() async {
    while (_rediscover) {
      _rediscover = false;
      // Cancel first, then let the captured Dart operation finish without
      // replacing/disposal of the opaque it may still reference after an await.
      _agent?.close();
      await _operation;
      await _metadataRead;
      if (!await _retireAgent(_agent)) return;
      if (!mounted) return;
      _cancelled = false;
      _publishing = false;
      // Coalesce changes received while the previous owner was draining. The
      // discovery below captures the latest client, not a queued intermediate.
      _rediscover = false;
      await _discoverOne();
      if (!mounted) {
        await _retireAgent(_agent);
        return;
      }
    }
  }

  Future<void> _discoverOne() async {
    final client = widget.client;
    final epoch = _epoch;
    _scope = _currentScope();
    native.View? view;
    try {
      view = client?.snapshot();
    } catch (error) {
      if (error is native.ClientError) error.dispose();
      if (mounted && epoch == _epoch) {
        setState(() {
          _busy = false;
          _error = 'unavailable';
        });
      }
      return;
    }
    if (client == null ||
        view == null ||
        !view.authenticated ||
        view.currentDeviceId.isEmpty) {
      if (mounted) {
        setState(() {
          _busy = false;
          _error = 'module_device_required';
        });
      }
      return;
    }
    if (mounted) {
      setState(() {
        _busy = true;
        _error = null;
      });
    }
    try {
      native.LocalAgent? agent;
      if (native.supportsLocalAgent()) {
        agent = await client.discoverLocalAgent();
      }
      if (!mounted || epoch != _epoch || !identical(client, widget.client)) {
        if (!await _retireAgent(agent)) return;
        _rediscover = mounted;
        return;
      }
      _agent = agent;
      await _refreshModules();
    } catch (error) {
      var code = 'unavailable';
      if (error is native.ClientError) {
        code = error.code();
        error.dispose();
      } else {
        widget.diagnostics?.reportFailure(native.FailureKind.nativeBridge);
      }
      if (mounted && epoch == _epoch) setState(() => _error = code);
    } finally {
      if (mounted && epoch == _epoch) {
        setState(() => _busy = _retiringAgent != null);
      }
    }
  }

  Future<void> _load(
    Future<Object> Function() operation, {
    bool cacheMutation = false,
  }) {
    if (_busy ||
        (cacheMutation && _cacheReadbackRequired) ||
        _agent == null ||
        _retiringAgent != null ||
        _transition != null) {
      return Future.value();
    }
    return _operation = _loadInner(operation, cacheMutation: cacheMutation);
  }

  Future<void> _loadInner(
    Future<Object> Function() operation, {
    required bool cacheMutation,
  }) async {
    final epoch = _epoch;
    setState(() {
      _busy = true;
      _cacheMutation = cacheMutation;
      _error = null;
      _result = null;
    });
    try {
      final result = await operation();
      if (mounted && epoch == _epoch) {
        setState(() {
          _result = result;
          if (result is native.CacheStatus) {
            _cacheReadbackRequired = false;
            _cacheCompletionUnconfirmed = false;
          }
        });
      }
    } catch (error) {
      var code = 'unknown';
      if (error is native.ClientError) {
        code = error.code();
        error.dispose();
      } else {
        widget.diagnostics?.reportFailure(native.FailureKind.nativeBridge);
      }
      if (mounted && epoch == _epoch) {
        setState(() {
          _error = code;
          // Public cancellation/revalidation or a bridge error does not reveal
          // whether an admitted write completed behind its publication fence.
          // Only these owner-specific pre-write refusals prove no new effect.
          if (cacheMutation &&
              !const {
                'cache_conflict',
                'cache_invalid',
                'cache_not_ready',
                'module_unsupported',
              }.contains(code)) {
            _cacheReadbackRequired = true;
            _cacheCompletionUnconfirmed = code != 'cache_uncertain';
          }
        });
      }
    } finally {
      if (mounted && epoch == _epoch) {
        setState(() {
          _busy = false;
          _cacheMutation = false;
          // Metadata refresh is separate from the completed operation result.
        });
        await _refreshModules();
      }
    }
  }

  void _cancel() {
    if (!_busy ||
        _cacheMutation ||
        _publishing ||
        _agent == null ||
        _cancelled ||
        _retiringAgent != null) {
      return;
    }
    _epoch++;
    _agent!.close();
    setState(() {
      _result = null;
      _error = 'cancelled';
      _cancelled = true;
    });
    _discover();
  }

  @override
  void dispose() {
    _epoch++;
    _rediscover = false;
    _agent?.close();
    _retiringAgent?.close();
    // Sync cancellation only. A retained transition handles any already-started
    // discovery/shutdown; dispose never invents a positive drain receipt.
    _repository.dispose();
    super.dispose();
  }

  String _name(AppLocalizations l, String id) => switch (id) {
    'sysinfo' => l.systemInfo,
    'clipboard' => l.clipboardHistory,
    'cleaner' => l.cleaner,
    'updater' => l.updater,
    'gds' => 'GDS',
    'rds' => 'RDS',
    'browser-use' => l.browserAutomation,
    'cache' => l.networkCache,
    _ => id,
  };

  String _errorMessage(AppLocalizations l, String error) => switch (error) {
    _ when _selected == 'cache' && _cacheReadbackRequired =>
      _cacheCompletionUnconfirmed
          ? l.cacheCompletionUnconfirmed
          : l.cacheStateUncertain,
    'unavailable' || 'unsupportedPlatform' => l.providerUnavailable,
    'busy' => l.rateError,
    'timeout' => l.moduleNetworkError,
    'cancelled' => l.moduleCancelled,
    'invalidInput' => l.moduleInputError,
    'invalidResponse' || 'invalidContract' => l.moduleResponseError,
    'sync_capacity_exhausted' => l.serverCapacityExhausted,
    'publish' => l.statePublishError,
    'module_device_required' => l.moduleDeviceRequired,
    'cache_uncertain' => l.cacheStateUncertain,
    _ => moduleFailureLabel(l, error),
  };

  String _number(num? value, [String unit = '']) => value == null
      ? AppLocalizations.of(context)!.missingReading
      : '${NumberFormat.decimalPatternDigits(locale: Localizations.localeOf(context).toString(), decimalDigits: 1).format(value)}$unit';

  Widget _rows(List<(String, String)> values) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: values
        .map(
          (row) => Padding(
            padding: const EdgeInsets.symmetric(
              vertical: OpenNetworkSpacing.two,
            ),
            child: Wrap(
              spacing: OpenNetworkSpacing.four,
              children: [Text('${row.$1}:'), SelectableText(row.$2)],
            ),
          ),
        )
        .toList(),
  );

  Widget _resultView(AppLocalizations l) => switch (_result) {
    native.BrowserStatus value => _rows([
      (
        l.browserOwnerSwitch,
        value.enabled ? l.browserSwitchEnabled : l.browserSwitchDisabled,
      ),
      (l.browserHostStatus, value.browser),
      (l.browserExtensionStatus, value.extension_),
      (l.browserSessionCount, '${value.sessions}'),
    ]),
    List<native.BrowserSession> value => Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(l.browserSessionScope),
        if (value.isEmpty) Text(l.noRecords),
        for (final session in value)
          ListTile(
            title: Text(session.project),
            subtitle: Text(l.browserTabCount(session.tabs.toString())),
          ),
      ],
    ),
    native.CacheStatus value => Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _rows([
          (l.cacheConfiguredPort, '${value.port}'),
          (
            l.cacheReachability,
            value.ready ? l.cacheProbeReady : l.cacheProbeUnavailable,
          ),
        ]),
        Text(l.cacheManagedScope),
        for (final binding in value.bindings)
          ListTile(
            title: Text(native.cacheBindingKind(binding: binding)),
            trailing: Text(
              native.cacheBindingBound(binding: binding)
                  ? l.cacheManagedPresent
                  : l.cacheManagedAbsent,
            ),
          ),
      ],
    ),
    sysinfo.SystemSnapshot value => _rows([
      (l.cpuUsage, _number(value.cpu.usage, '%')),
      (l.cpuTemperature, _number(value.cpu.temp, ' °C')),
      (l.memoryUsage, _number(value.memory.used, '%')),
      (l.swapUsage, _number(value.memory.swap, '%')),
      (l.gpuUsage, _number(value.gpu.usage, '%')),
      (l.gpuMemory, _number(value.gpu.memory, '%')),
      (l.gpuTemperature, _number(value.gpu.temp, ' °C')),
      (l.diskRead, _number(value.disk.read?.toDouble(), ' B/s')),
      (l.diskWrite, _number(value.disk.write?.toDouble(), ' B/s')),
      (l.diskTemperature, _number(value.disk.temp, ' °C')),
      (l.networkReceive, _number(value.net.rx?.toDouble(), ' B/s')),
      (l.networkSend, _number(value.net.tx?.toDouble(), ' B/s')),
    ]),
    clipboard.Page value => Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (value.items.isEmpty) Text(l.noRecords),
        for (final entry in value.items)
          ListTile(
            title: Text(
              entry.preview ?? entry.kind,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
            subtitle: Text('${entry.bytes} B · ${entry.mimes.join(', ')}'),
            trailing:
                entry.mimes.any(
                  (mime) =>
                      mime == 'text/plain' || mime.startsWith('text/plain;'),
                )
                ? IconButton(
                    tooltip: l.openText,
                    icon: const Icon(Icons.article_outlined),
                    onPressed: _busy
                        ? null
                        : () => _load(() async {
                            final mime = entry.mimes.firstWhere(
                              (mime) =>
                                  mime == 'text/plain' ||
                                  mime.startsWith('text/plain;'),
                            );
                            final bytes = await _agent!.clipboardContent(
                              client: widget.client!,
                              entry: entry.id,
                              mime: mime,
                            );
                            return utf8.decode(
                              bytes.take(16_384).toList(),
                              allowMalformed: true,
                            );
                          }),
                  )
                : null,
          ),
        if (value.more && value.items.isNotEmpty)
          TextButton(
            onPressed: _busy
                ? null
                : () => _load(
                    () => _agent!.clipboardEntries(
                      client: widget.client!,
                      before: value.items.last.id,
                    ),
                  ),
            child: Text(l.nextPage),
          ),
      ],
    ),
    cleaner.Diagnostics value => Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _rows([
          (l.errors, '${value.errors}'),
          (l.warnings, '${value.warnings}'),
        ]),
        for (final check in value.checks)
          ListTile(
            title: Text(check.id),
            subtitle: Text(check.detail),
            trailing: Text(switch (check.state) {
              cleaner.CheckState.ok => l.checkOk,
              cleaner.CheckState.warning => l.checkWarning,
              cleaner.CheckState.error => l.checkError,
              cleaner.CheckState.info => l.checkInfo,
            }),
          ),
      ],
    ),
    cleaner.CleanerRunSummary value => _rows([
      (l.mode, value.dryRun ? l.preview : l.executed),
      (l.duration, '${value.durationMs} ms'),
      (
        l.freedBytes,
        value.freedBytes == null ? l.unavailable : '${value.freedBytes} B',
      ),
      (l.deleted, '${value.deleted}'),
      (l.skipped, '${value.skipped}'),
      (l.failed, '${value.failed}'),
    ]),
    updater.Plan value => Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('${l.channel}: ${value.channel}'),
        if (value.actions.isEmpty) Text(l.noRecords),
        for (final action in value.actions)
          ListTile(
            title: Text(switch (native.updateActionKind(action: action)) {
              'gds_install' => l.gdsInstall,
              'verified_applications' => l.verifiedApplications,
              'apt_observe' => l.aptObserve,
              'toolchain_observe' => l.toolchainObserve,
              'homebrew' => 'Homebrew',
              'flatpak' => 'Flatpak',
              _ => l.unavailable,
            }),
            subtitle: Text(native.updateActionPlatform(action: action) ?? ''),
          ),
      ],
    ),
    updater.UpdateRunSummary value => Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('${l.channel}: ${value.channel}'),
        if (value.results.isEmpty) Text(l.noRecords),
        for (final action in value.results)
          ListTile(
            title: Text(action.action),
            subtitle: Text(action.detail),
            trailing: Text(action.ok ? l.checkOk : l.checkError),
          ),
      ],
    ),
    gds.Status value => Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _rows([(l.result, value.result), (l.exitClass, value.exitClass)]),
        if (value.status case final status?)
          _rows([
            (l.branch, status.branch.name ?? l.gdsUnavailable),
            (l.upstream, status.branch.upstream ?? l.gdsUnavailable),
            (
              l.aheadBehind,
              switch (status.branch.comparisonState) {
                'available' =>
                  '${status.branch.ahead} / ${status.branch.behind}',
                'not-applicable' => l.gdsComparisonNotApplicable,
                _ => l.gdsUnavailable,
              },
            ),
            (l.staged, '${status.changes.staged}'),
            (l.unstaged, '${status.changes.unstaged}'),
            (l.untracked, '${status.changes.untracked}'),
            (l.conflicts, '${status.changes.conflicted}'),
            (l.remoteFreshness, status.remoteFreshness),
          ]),
        for (final finding in value.findings)
          ListTile(title: Text(finding.code), subtitle: Text(finding.message)),
      ],
    ),
    rds.RemoteSnapshot value => Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (value.sessions.isEmpty) Text(l.noRecords),
        for (final session in value.sessions)
          ListTile(
            title: Text(session.peer),
            subtitle: Text(session.id),
            trailing: Text(
              session.status == rds.SessionState.connected
                  ? l.connected
                  : l.connecting,
            ),
          ),
      ],
    ),
    String value => Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [Text(l.textPreviewLimit), SelectableText(value)],
    ),
    _ => const SizedBox.shrink(),
  };

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context)!;
    final client = widget.client;
    final scope = _scope;
    final current = _modules
        .where((module) => module.id == _selected)
        .firstOrNull;
    Widget action(
      String label,
      Future<Object> Function() run, {
      Key? key,
      bool cacheMutation = false,
    }) => FilledButton(
      key: key,
      onPressed:
          _busy ||
              (cacheMutation && _cacheReadbackRequired) ||
              _agent == null ||
              current?.localToolSupported != true ||
              current?.localToolInstalled != true
          ? null
          : () => _load(run, cacheMutation: cacheMutation),
      child: Text(label),
    );
    return Scaffold(
      appBar: AppBar(
        title: const OpenNetworkBrand(),
        toolbarHeight:
            OpenNetworkBrand.heightOf(context) + OpenNetworkSpacing.four,
        actions: [
          PopupMenuButton<String>(
            tooltip: 'English / Русский',
            onSelected: (value) => widget.onLocale(Locale(value)),
            itemBuilder: (_) => const [
              PopupMenuItem(value: 'en', child: Text('English')),
              PopupMenuItem(value: 'ru', child: Text('Русский')),
            ],
            icon: const Icon(Icons.language),
          ),
        ],
      ),
      body: SafeArea(
        child: Align(
          alignment: Alignment.topCenter,
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(OpenNetworkSpacing.six),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 900),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(l.moduleManagementHelp),
                  if (widget.client != null)
                    OutlinedButton(
                      key: const Key('module-settings-refresh'),
                      onPressed: _refreshModules,
                      child: Text(l.moduleReloadDraft),
                    ),
                  if (_metadataError)
                    Text(
                      l.moduleMetadataError,
                      key: const Key('module-metadata-error'),
                    ),
                  if (widget.client?.snapshot().currentDeviceId.isNotEmpty ==
                      true) ...[
                    OutlinedButton(
                      key: const Key('publish-local-state'),
                      onPressed: _busy || _agent == null ? null : _publish,
                      child: Text(l.publishModuleState),
                    ),
                    if (_published) Text(l.moduleStateQueued),
                  ],
                  const SizedBox(height: OpenNetworkSpacing.four),
                  if (_modules.isNotEmpty)
                    DropdownButtonFormField<String>(
                      key: const Key('module-selector'),
                      isExpanded: true,
                      isDense: false,
                      itemHeight: null,
                      initialValue: _selected,
                      items: _modules
                          .map(
                            (module) => DropdownMenuItem(
                              value: module.id,
                              child: Text(_name(l, module.id)),
                            ),
                          )
                          .toList(),
                      onChanged: _busy
                          ? null
                          : (value) => setState(() {
                              _selected = value!;
                              _result = null;
                              _error = null;
                            }),
                      decoration: InputDecoration(labelText: l.module),
                    ),
                  if (current != null)
                    Padding(
                      padding: const EdgeInsets.symmetric(
                        vertical: OpenNetworkSpacing.four,
                      ),
                      child: Text(
                        '${_name(l, current.id)} · ${current.version}${current.checkedAtMs == null ? '' : ' · ${DateFormat.yMd(Localizations.localeOf(context).toString()).add_Hms().format(DateTime.fromMillisecondsSinceEpoch(current.checkedAtMs!.toInt()))}'}',
                      ),
                    ),
                  if (current != null && widget.client != null)
                    ModuleSettings(
                      key: ValueKey('$_scope:${current.id}'),
                      client: widget.client!,
                      module: current,
                      onSaved: () async {
                        // A read begun before this edit must finish first. The
                        // following read then observes the durable new basis.
                        await _metadataRead;
                        if (!mounted ||
                            !identical(client, widget.client) ||
                            scope != _scope) {
                          return;
                        }
                        if (_selected == current.id) {
                          setState(() => _result = null);
                        }
                        await _refreshModules();
                      },
                    ),
                  if (_selected == 'gds')
                    Padding(
                      padding: const EdgeInsets.only(
                        bottom: OpenNetworkSpacing.four,
                      ),
                      child: TextField(
                        key: const Key('repository-path'),
                        controller: _repository,
                        enabled: !_busy,
                        decoration: InputDecoration(
                          labelText: l.repositoryPath,
                        ),
                      ),
                    ),
                  Wrap(
                    spacing: OpenNetworkSpacing.two,
                    runSpacing: OpenNetworkSpacing.two,
                    children: [
                      if (_selected == 'sysinfo')
                        action(
                          l.moduleRefresh,
                          () => _agent!.systemInfo(client: widget.client!),
                          key: const Key('read-system-info'),
                        ),
                      if (_selected == 'clipboard')
                        action(
                          l.moduleRefresh,
                          () =>
                              _agent!.clipboardEntries(client: widget.client!),
                        ),
                      if (_selected == 'cleaner') ...[
                        action(
                          l.moduleDiagnostics,
                          () => _agent!.cleanerDiagnostics(
                            client: widget.client!,
                          ),
                        ),
                        action(
                          l.preview,
                          () => _agent!.cleanerPreview(client: widget.client!),
                        ),
                        action(
                          l.lastRun,
                          () => _agent!.cleanerLastRun(client: widget.client!),
                        ),
                      ],
                      if (_selected == 'updater') ...[
                        action(
                          l.updatePlan,
                          () => _agent!.updatePlan(client: widget.client!),
                        ),
                        action(
                          l.lastRun,
                          () => _agent!.updateLastRun(client: widget.client!),
                        ),
                      ],
                      if (_selected == 'gds')
                        action(
                          l.moduleRefresh,
                          () => _agent!.repositoryStatus(
                            client: widget.client!,
                            path: _repository.text.trim(),
                          ),
                        ),
                      if (_selected == 'rds')
                        action(
                          l.moduleRefresh,
                          () => _agent!.remoteSessions(client: widget.client!),
                        ),
                      if (_selected == 'browser-use') ...[
                        action(l.browserAllowWork, () async {
                          await _agent!.resumeBrowser(client: widget.client!);
                          return _agent!.browserStatus(client: widget.client!);
                        }, key: const Key('browser-allow-work')),
                        action(
                          l.moduleRefresh,
                          () => _agent!.browserStatus(client: widget.client!),
                          key: const Key('browser-status'),
                        ),
                        action(
                          l.browserSessionCount,
                          () => _agent!.browserSessions(client: widget.client!),
                          key: const Key('browser-sessions'),
                        ),
                      ],
                      if (_selected == 'cache') ...[
                        action(
                          l.moduleRefresh,
                          () => _agent!.cacheStatus(client: widget.client!),
                          key: const Key('cache-status'),
                        ),
                        action(
                          l.cacheBindTools,
                          () => _agent!.cacheBind(client: widget.client!),
                          key: const Key('cache-bind'),
                          cacheMutation: true,
                        ),
                        action(
                          l.cacheUnbindTools,
                          () => _agent!.cacheUnbind(client: widget.client!),
                          key: const Key('cache-unbind'),
                          cacheMutation: true,
                        ),
                      ],
                      if (_busy &&
                          !_publishing &&
                          !_cacheMutation &&
                          _agent != null)
                        TextButton(
                          onPressed: _cancelled ? null : _cancel,
                          child: Text(l.moduleCancel),
                        ),
                      if (_agent == null && !_busy)
                        TextButton(
                          onPressed: () {
                            setState(() => _busy = true);
                            _discover();
                          },
                          child: Text(l.moduleRefresh),
                        ),
                    ],
                  ),
                  if (_busy && _cacheMutation && !_cleanupHeld)
                    Text(l.cacheMutationWorking),
                  if (_selected == 'cache' &&
                      _cacheReadbackRequired &&
                      !_cleanupHeld &&
                      _error == null)
                    Text(
                      _cacheCompletionUnconfirmed
                          ? l.cacheCompletionUnconfirmed
                          : l.cacheStateUncertain,
                    ),
                  if (_busy && !_cleanupHeld) const LinearProgressIndicator(),
                  if (_error != null)
                    Padding(
                      padding: const EdgeInsets.symmetric(
                        vertical: OpenNetworkSpacing.four,
                      ),
                      child: Semantics(
                        liveRegion: true,
                        child: Text(
                          _cleanupHeld
                              ? l.moduleShutdownUnconfirmed
                              : _errorMessage(l, _error!),
                          key: const Key('module-error'),
                          style: TextStyle(
                            color: Theme.of(context).colorScheme.error,
                          ),
                        ),
                      ),
                    ),
                  const SizedBox(height: OpenNetworkSpacing.six),
                  _resultView(l),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
