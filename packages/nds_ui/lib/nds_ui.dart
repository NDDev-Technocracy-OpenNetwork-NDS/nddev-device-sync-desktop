import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart'
    show kIsWeb, defaultTargetPlatform, TargetPlatform;
import 'package:flutter/services.dart';
import 'package:flutter/semantics.dart';
import 'package:nddev_opennetwork_design_system/opennetwork_design_system.dart';
import 'package:nds_native/nds_native.dart' as native;
import 'package:url_launcher/url_launcher.dart';

import 'l10n/app_localizations.dart';
import 'activity.dart';
import 'app_update.dart';
import 'modules_screen.dart';
import 'screen_viewer.dart';
import 'screen_requests.dart';
import 'diagnostics.dart';
import 'vault_screen.dart';
import 'utilities_screen.dart';
import 'passkeys_screen.dart';
import 'company_screen.dart';
import 'repositories_screen.dart';

Future<void> runNdsApp() async {
  WidgetsFlutterBinding.ensureInitialized();
  final invitationReceived = kIsWeb && Uri.base.fragment.startsWith('invite/');
  final diagnostics = ApplicationDiagnostics();
  var nativeReady = false;
  try {
    await native.initializeNdsBridge();
    diagnostics.nativeInitialized();
    nativeReady = true;
  } catch (_) {
    diagnostics.reportFailure(native.FailureKind.nativeBridge);
  }
  runApp(
    NdsApp(
      diagnostics: diagnostics,
      nativeReady: nativeReady,
      invitationReceived: invitationReceived,
    ),
  );
}

class NdsApp extends StatefulWidget {
  const NdsApp({
    super.key,
    this.diagnostics,
    this.nativeReady = true,
    this.invitationReceived = false,
  });
  final ApplicationDiagnostics? diagnostics;
  final bool nativeReady;
  final bool invitationReceived;
  @override
  State<NdsApp> createState() => _NdsAppState();
}

class _NdsAppState extends State<NdsApp> with WidgetsBindingObserver {
  final _identityKey = GlobalKey<_IdentityScreenState>();
  SemanticsHandle? _webSemantics;
  Locale? _locale;
  int _page = 0;
  native.NdsClient? _client;
  late final ApplicationActivity _activity;
  @override
  void initState() {
    super.initState();
    // Expose ordinary controls to assistive tools and browser agents without
    // requiring Flutter's off-screen activation button. The app owns the handle.
    if (kIsWeb) _webSemantics = SemanticsBinding.instance.ensureSemantics();
    WidgetsBinding.instance.addObserver(this);
    _activity = ApplicationActivity(
      nativeReady: widget.nativeReady,
      onLifecycle: widget.diagnostics?.lifecycle,
      onBridgeFailure: () =>
          widget.diagnostics?.reportFailure(native.FailureKind.nativeBridge),
    );
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && widget.nativeReady) widget.diagnostics?.clientReady();
    });
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _webSemantics?.dispose();
    _activity.dispose();
    widget.diagnostics?.dispose();
    super.dispose();
  }

  @override
  Future<bool> didPushRouteInformation(
    RouteInformation routeInformation,
  ) async {
    final uri = routeInformation.uri;
    if (!kIsWeb ||
        !(uri.fragment.startsWith('invite/') ||
            uri.path.startsWith('/invite/') ||
            uri.path.startsWith('invite/'))) {
      return false;
    }
    await _identityKey.currentState?._receiveInvitation(uri.toString());
    return true;
  }

  @override
  Widget build(BuildContext context) => ApplicationActivityScope(
    activity: _activity,
    child: LayoutBuilder(
      builder: (context, constraints) => MaterialApp(
        title: 'NDDev OpenNetwork',
        initialRoute: '/',
        debugShowCheckedModeBanner: false,
        theme: OpenNetworkTheme.dark(width: constraints.maxWidth),
        locale: _locale,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: Builder(
          builder: (context) {
            widget.diagnostics?.locale = Localizations.localeOf(context);
            if (!widget.nativeReady) {
              return Scaffold(
                appBar: AppBar(
                  title: const OpenNetworkBrand(),
                  toolbarHeight:
                      OpenNetworkBrand.heightOf(context) +
                      OpenNetworkSpacing.four,
                ),
                body: SafeArea(
                  child: Padding(
                    padding: const EdgeInsets.all(OpenNetworkSpacing.six),
                    child: Text(AppLocalizations.of(context)!.startupFailed),
                  ),
                ),
              );
            }
            final identity = _IdentityScreen(
              key: _identityKey,
              invitationReceived: widget.invitationReceived,
              onLocale: (locale) => setState(() => _locale = locale),
              diagnostics: widget.diagnostics,
              onClient: (client) {
                _activity.attach(client);
                setState(() => _client = client);
              },
            );
            final l = AppLocalizations.of(context)!;
            return Scaffold(
              body: IndexedStack(
                index: _page,
                children: [
                  identity,
                  ModulesScreen(
                    onLocale: (locale) => setState(() => _locale = locale),
                    diagnostics: widget.diagnostics,
                    client: _client,
                  ),
                ],
              ),
              bottomNavigationBar: NavigationBar(
                selectedIndex: _page,
                onDestinationSelected: (page) => setState(() => _page = page),
                destinations: [
                  NavigationDestination(
                    icon: const Icon(Icons.account_circle_outlined),
                    label: l.sessionTab,
                  ),
                  NavigationDestination(
                    icon: const Icon(Icons.devices),
                    label: l.localModules,
                  ),
                ],
              ),
            );
          },
        ),
      ),
    ),
  );
}

class _IdentityScreen extends StatefulWidget {
  const _IdentityScreen({
    super.key,
    this.invitationReceived = false,
    required this.onLocale,
    this.diagnostics,
    this.onClient,
  });
  final bool invitationReceived;
  final ValueChanged<Locale> onLocale;
  final ApplicationDiagnostics? diagnostics;
  final ValueChanged<native.NdsClient?>? onClient;
  @override
  State<_IdentityScreen> createState() => _IdentityScreenState();
}

class _IdentityScreenState extends State<_IdentityScreen> {
  final _endpoint = TextEditingController(
    text: kIsWeb
        ? Uri.base.origin
        : const String.fromEnvironment('NDS_SERVER_URL'),
  );
  final _email = TextEditingController();
  final _code = TextEditingController();
  native.NdsClient? _client;
  native.View? _view;
  bool _busy = false;
  int _actionEpoch = 0;
  bool get _paused => !ApplicationActivityScope.read(context).value;
  bool _browserOpened = false;
  bool _invitationNoticeHandled = false;
  String? _error;
  Timer? _poll;
  Timer? _clock;
  int _reconcileTick = 0;
  Future<void>? _reconciliation;
  native.DebugScope _debugScope = native.DebugScope.http;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && _endpoint.text.trim().isNotEmpty) unawaited(_connect());
    });
    _clock = Timer.periodic(const Duration(seconds: 1), (_) {
      if (mounted && !_paused) {
        setState(() => _view = _client?.snapshot());
        if (++_reconcileTick % 5 == 0) {
          _reconcile();
          if (!_busy && _client != null) widget.onClient?.call(_client);
        }
      }
    });
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    ApplicationActivityScope.of(context);
    if (_paused) {
      _poll?.cancel();
    } else {
      _reconcile();
      _schedulePoll();
    }
  }

  void _reconcile() {
    if (_paused || _busy || _reconciliation != null || _client == null) return;
    _reconciliation = _reconcileInner(_client!).whenComplete(() {
      _reconciliation = null;
    });
  }

  Future<void> _reconcileInner(native.NdsClient client) async {
    try {
      await client.reconcile();
      if (kIsWeb &&
          !client.snapshot().authenticated &&
          client.snapshot().flow.isEmpty) {
        await client.importBrowserInvitation();
      }
    } catch (error) {
      if (error is native.ClientError) {
        error.dispose();
      } else {
        widget.diagnostics?.reportFailure(native.FailureKind.nativeBridge);
      }
    } finally {
      if (mounted && identical(client, _client)) {
        setState(() => _view = client.snapshot());
      }
    }
  }

  @override
  void dispose() {
    _clock?.cancel();
    _poll?.cancel();
    _client?.cancel();
    _client?.dispose();
    _endpoint.dispose();
    _email.dispose();
    _code.clear();
    _code.dispose();
    super.dispose();
  }

  String get _locale => Localizations.localeOf(context).languageCode;

  Future<void> _run(
    Future<native.View> Function() action, {
    bool prepare = false,
  }) async {
    if (_busy) return;
    final epoch = _actionEpoch;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await _reconciliation;
      var view = await action();
      if (view.authenticated &&
          (view.sessionVerified || view.offlineAuthorized) &&
          (prepare || view.currentDeviceId.isEmpty) &&
          view.flow != 'passkey_registration') {
        view = await _client!.prepareDevice(
          name: const String.fromEnvironment('NDS_DEVICE_NAME'),
        );
      }
      if (!mounted || epoch != _actionEpoch) return;
      setState(() => _view = view);
      widget.onClient?.call(_client);
      if (view.authenticated) _code.clear();
    } catch (error) {
      var code = 'unexpected';
      if (error is native.ClientError) {
        code = error.code();
        error.dispose();
      } else {
        widget.diagnostics?.reportFailure(native.FailureKind.nativeBridge);
      }
      if (mounted && epoch == _actionEpoch) {
        setState(() {
          _error = code;
          _view = _client?.snapshot();
        });
      }
    } finally {
      if (mounted) {
        setState(() {
          _busy = false;
          _view = _client?.snapshot();
        });
        _schedulePoll();
      }
    }
  }

  Future<void> _connect() => _run(() async {
    widget.onClient?.call(null);
    _client?.cancel();
    _client?.dispose();
    _client = null;
    _view = null;
    final epoch = _actionEpoch;
    final client = native.NdsClient(endpoint: _endpoint.text.trim());
    if (!mounted || epoch != _actionEpoch) {
      client.cancel();
      client.dispose();
      throw StateError('cancelled');
    }
    ApplicationActivityScope.read(context).attach(client);
    _client = client;
    await client.refresh();
    final view = await client.importBrowserInvitation();
    if (widget.invitationReceived &&
        !_invitationNoticeHandled &&
        view.authenticated) {
      _invitationNoticeHandled = true;
      _error = 'invitation_requires_sign_out';
    }
    return view;
  }, prepare: true);

  void _schedulePoll() {
    _poll?.cancel();
    if (!_browserOpened ||
        _paused ||
        _busy ||
        !{'github', 'passkey', 'passkey_registration'}.contains(_view?.flow) ||
        _view!.expiresInSeconds == 0) {
      return;
    }
    _poll = Timer(Duration(seconds: _view!.retryAfterSeconds.clamp(1, 30)), () {
      if (mounted && !_paused && _client != null) {
        _run(
          () => _view?.flow == 'github'
              ? _client!.pollGithub()
              : _client!.pollPasskey(),
        );
      }
    });
  }

  Future<void> _receiveInvitation(String route) async {
    try {
      native.captureInvitationLink(route: route);
    } catch (error) {
      if (error is native.ClientError) error.dispose();
      if (mounted) setState(() => _error = 'invalid_input');
      return;
    }
    if (_client?.snapshot().authenticated == true) {
      if (mounted) setState(() => _error = 'invitation_requires_sign_out');
      return;
    }
    if (_client != null && !_busy) {
      await _run(() => _client!.importBrowserInvitation());
    }
  }

  Future<void> _importInvitation() async {
    final l = AppLocalizations.of(context)!;
    final input = TextEditingController();
    try {
      final accepted = await showDialog<bool>(
        context: context,
        builder: (context) => AlertDialog(
          title: Text(l.invitationImport),
          content: TextField(
            controller: input,
            obscureText: true,
            autocorrect: false,
            enableSuggestions: false,
            maxLength: 2048,
            decoration: InputDecoration(labelText: l.invitationLink),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(context, false),
              child: Text(l.cancel),
            ),
            FilledButton(
              onPressed: () => Navigator.pop(context, true),
              child: Text(l.invitationImport),
            ),
          ],
        ),
      );
      if (accepted == true && mounted) {
        final link = input.text;
        input.clear();
        await _run(() => _client!.importInvitation(link: link));
      }
    } finally {
      input.clear();
      input.dispose();
    }
  }

  Future<void> _startGithub() {
    _browserOpened = false;
    _poll?.cancel();
    return _run(() => _client!.startGithub(locale: _locale));
  }

  Future<void> _startPasskey(bool registration) {
    _browserOpened = false;
    _poll?.cancel();
    return _run(
      () => _client!.startPasskey(
        registration: registration,
        email: registration || _email.text.trim().isEmpty
            ? null
            : _email.text.trim(),
        locale: _locale,
      ),
    );
  }

  Future<void> _openGithub() async {
    final epoch = _actionEpoch;
    final uri = Uri.tryParse(_view?.authorizationUrl ?? '');
    // Defense in depth for the only externally launched authorization URL.
    final endpoint = Uri.tryParse(_endpoint.text.trim());
    final passkey = _view?.flow != 'github';
    final validPasskey =
        endpoint != null &&
        uri != null &&
        uri.origin == endpoint.origin &&
        uri.path == '/v2/auth/passkeys/browser' &&
        uri.userInfo.isEmpty &&
        !uri.hasFragment;
    if (uri == null ||
        (passkey
            ? !validPasskey
            : (uri.scheme != 'https' ||
                  uri.host != 'github.com' ||
                  uri.path != '/login/oauth/authorize' ||
                  uri.userInfo.isNotEmpty ||
                  uri.hasFragment ||
                  uri.port != 443))) {
      setState(() => _error = 'invalid_response');
      return;
    }
    try {
      if (!await launchUrl(uri, mode: LaunchMode.externalApplication)) {
        throw StateError('unavailable');
      }
      if (!mounted || epoch != _actionEpoch) return;
      _browserOpened = true;
      _schedulePoll();
    } catch (_) {
      widget.diagnostics?.reportFailure(native.FailureKind.nativeBridge);
      if (mounted) setState(() => _error = 'browser_unavailable');
    }
  }

  void _cancel() {
    _actionEpoch++;
    _poll?.cancel();
    _browserOpened = false;
    _client?.cancel();
    _code.clear();
    setState(() {
      _view = _client?.snapshot();
      _error = 'cancelled';
    });
  }

  String _message(AppLocalizations l, String code) => switch (code) {
    'invitation_requires_sign_out' => l.invitationSignOut,
    'invalid_endpoint' => l.endpointError,
    'recent_authentication_required' => l.recentAuthenticationRequired,
    'invalid_input' => l.inputError,
    'credential_store_unavailable' => l.storeError,
    'vault_storage_unavailable' => l.vaultStorageUnavailable,
    'vault_storage_full' => l.storageFull,
    'credential_revalidation_required' => l.revalidationRequired,
    'transport_unavailable' ||
    'request_timeout' ||
    'dependency_unavailable' => l.networkError,
    'authentication_failed' || 'authorization_denied' => l.authError,
    'device_revoked' => l.deviceRevokedHelp,
    'challenge_expired' => l.expiredError,
    'rate_limited' ||
    'server_busy' ||
    'busy' ||
    'online_verification_required' => l.rateError,
    'cancelled' => l.cancelledError,
    'invalid_response' => l.responseError,
    'browser_unavailable' => l.launchError,
    'method_unavailable' => l.unavailable,
    'diagnostics_unavailable' => l.diagnosticsUnavailable,
    _ => l.genericError,
  };

  String _date(String value) {
    final date = DateTime.tryParse(value)?.toLocal();
    if (date == null) return value;
    final format = MaterialLocalizations.of(context);
    return '${format.formatShortDate(date)} ${format.formatTimeOfDay(TimeOfDay.fromDateTime(date))}';
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context)!;
    final v = _view;
    final diagnostics = native.clientDiagnostics();
    return Scaffold(
      appBar: AppBar(
        title: const OpenNetworkBrand(),
        toolbarHeight:
            OpenNetworkBrand.heightOf(context) + OpenNetworkSpacing.four,
        actions: [
          if (!kIsWeb && defaultTargetPlatform == TargetPlatform.linux)
            IconButton(
              key: const Key('app-update-open'),
              tooltip: l.appUpdateTitle,
              icon: const Icon(Icons.system_update_alt),
              onPressed: () => Navigator.of(context).push<void>(
                MaterialPageRoute(builder: (_) => const AppUpdateScreen()),
              ),
            ),
          PopupMenuButton<String>(
            tooltip: 'English / Русский',
            onSelected: (language) => widget.onLocale(Locale(language)),
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
              constraints: const BoxConstraints(maxWidth: 640),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(
                    v?.authenticated == true
                        ? l.sessionTitle
                        : _client == null || v?.serverVersion.isEmpty != false
                        ? l.connectTitle
                        : l.signIn,
                    style: Theme.of(context).textTheme.headlineSmall,
                  ),
                  const SizedBox(height: OpenNetworkSpacing.six),
                  if (v?.logoutPending == true) Text(l.logoutPending),
                  if (_client == null || v?.serverVersion.isEmpty != false) ...[
                    Text(l.connectHelp),
                    const SizedBox(height: OpenNetworkSpacing.four),
                    TextField(
                      key: const Key('server-address'),
                      controller: _endpoint,
                      enabled: !_busy,
                      readOnly: kIsWeb,
                      keyboardType: TextInputType.url,
                      autocorrect: false,
                      decoration: InputDecoration(
                        labelText: l.serverAddress,
                        hintText: 'https://server.example',
                      ),
                      onSubmitted: (_) {
                        if (!_busy) _connect();
                      },
                    ),
                    const SizedBox(height: OpenNetworkSpacing.four),
                    FilledButton(
                      key: const Key('connect'),
                      onPressed: _busy ? null : _connect,
                      child: Text(l.connect),
                    ),
                  ] else if (v!.credentialRevalidationRequired ||
                      !v.credentialStoreReady) ...[
                    Text(
                      v.credentialRevalidationRequired
                          ? l.revalidationRequired
                          : l.storeError,
                    ),
                    const SizedBox(height: OpenNetworkSpacing.four),
                    FilledButton(
                      key: const Key('refresh'),
                      onPressed: _busy
                          ? null
                          : () => _run(() => _client!.refresh(), prepare: true),
                      child: Text(l.refresh),
                    ),
                  ] else if (v.authenticated &&
                      v.flow != 'passkey_registration') ...[
                    if (!v.sessionVerified) ...[
                      Text(
                        v.offlineAuthorized
                            ? l.offlineAuthorized
                            : l.unverified,
                      ),
                      const SizedBox(height: OpenNetworkSpacing.four),
                    ],
                    SelectableText(
                      '${l.instance}: ${_endpoint.text}\n${l.user}: ${v.userId}\n${l.method}: ${switch (v.authMethod) {
                        'email_otp' => l.emailMethod,
                        'passkey' => l.passkeyMethod,
                        _ => l.github,
                      }}\n${l.expires}: ${_date(v.expiresAt)}',
                    ),
                    const SizedBox(height: OpenNetworkSpacing.four),
                    Text(
                      '${l.verified}: ${_date(DateTime.fromMillisecondsSinceEpoch(v.sessionCheckedAt.toInt() * 1000, isUtc: true).toIso8601String())}',
                    ),
                    const SizedBox(height: OpenNetworkSpacing.six),
                    Text(
                      l.devicesTitle,
                      style: Theme.of(context).textTheme.titleLarge,
                    ),
                    Text(l.devicesHelp),
                    const SizedBox(height: OpenNetworkSpacing.four),
                    if (v.devicesCheckedAt == 0) Text(l.devicesUnverified),
                    for (final device in v.devices)
                      Card(
                        child: Padding(
                          padding: const EdgeInsets.all(
                            OpenNetworkSpacing.four,
                          ),
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(
                                device.displayName,
                                style: Theme.of(context).textTheme.titleMedium,
                              ),
                              Text(
                                '${device.platform} · ${device.revoked ? l.deviceRevoked : l.deviceActive}',
                              ),
                              if (!device.isCurrent &&
                                  !device.revoked &&
                                  native.screenSupported())
                                TextButton(
                                  onPressed: _busy
                                      ? null
                                      : () => Navigator.of(context).push(
                                          MaterialPageRoute<void>(
                                            builder: (_) => ScreenViewer(
                                              client: _client!,
                                              deviceId: device.deviceId,
                                            ),
                                          ),
                                        ),
                                  child: Text(l.screenView),
                                ),
                              if (device.isCurrent &&
                                  !device.revoked &&
                                  native.screenSupported())
                                TextButton(
                                  onPressed: _busy
                                      ? null
                                      : () => Navigator.of(context).push(
                                          MaterialPageRoute<void>(
                                            builder: (_) => ScreenRequests(
                                              client: _client!,
                                            ),
                                          ),
                                        ),
                                  child: Text(l.screenRequests),
                                ),
                              if (device.isCurrent) Text(l.thisDevice),
                              Text(
                                '${l.deviceCreated}: ${_date(device.createdAt)}',
                              ),
                              if (device.revoked && device.isCurrent)
                                Text(l.deviceRevokedHelp),
                              if (!device.revoked)
                                TextButton(
                                  key: Key('revoke-device-${device.deviceId}'),
                                  onPressed: _busy || !v.sessionVerified
                                      ? null
                                      : () async {
                                          final confirmed =
                                              await showDialog<bool>(
                                                context: context,
                                                builder: (context) =>
                                                    AlertDialog(
                                                      title: Text(
                                                        l.revokeDevice,
                                                      ),
                                                      content: Text(
                                                        l.revokeDeviceHelp(
                                                          device.displayName,
                                                        ),
                                                      ),
                                                      actions: [
                                                        TextButton(
                                                          onPressed: () =>
                                                              Navigator.pop(
                                                                context,
                                                                false,
                                                              ),
                                                          child: Text(
                                                            l.keepDevice,
                                                          ),
                                                        ),
                                                        FilledButton(
                                                          onPressed: () =>
                                                              Navigator.pop(
                                                                context,
                                                                true,
                                                              ),
                                                          child: Text(
                                                            l.revokeDevice,
                                                          ),
                                                        ),
                                                      ],
                                                    ),
                                              );
                                          if (confirmed == true && mounted) {
                                            await _run(
                                              () => _client!.revokeDevice(
                                                id: device.deviceId,
                                              ),
                                            );
                                          }
                                        },
                                  child: Text(l.revokeDevice),
                                ),
                            ],
                          ),
                        ),
                      ),
                    TextButton(
                      key: const Key('refresh-devices'),
                      onPressed: _busy || !v.sessionVerified
                          ? null
                          : () => _run(() => _client!.refreshDevices()),
                      child: Text(l.refreshDevices),
                    ),
                    if (v.devices.any(
                      (device) => device.isCurrent && !device.revoked,
                    ))
                      FilledButton(
                        key: const Key('open-vault'),
                        onPressed:
                            _busy || !(v.sessionVerified || v.offlineAuthorized)
                            ? null
                            : () => Navigator.of(context).push(
                                MaterialPageRoute<void>(
                                  builder: (_) => VaultScreen(client: _client!),
                                ),
                              ),
                        child: Text(l.vaultTitle),
                      ),
                    const SizedBox(height: OpenNetworkSpacing.four),
                    OutlinedButton(
                      key: const Key('open-utilities'),
                      onPressed:
                          _busy || !(v.sessionVerified || v.offlineAuthorized)
                          ? null
                          : () => Navigator.of(context).push(
                              MaterialPageRoute<void>(
                                builder: (_) =>
                                    UtilitiesScreen(client: _client!),
                              ),
                            ),
                      child: Text(l.utilitiesTitle),
                    ),
                    OutlinedButton(
                      key: const Key('open-github-catalog'),
                      onPressed: _busy || _paused
                          ? null
                          : () => Navigator.of(context).push(
                              MaterialPageRoute<void>(
                                builder: (_) =>
                                    RepositoriesScreen(client: _client!),
                              ),
                            ),
                      child: Text(l.catalogTitle),
                    ),
                    OutlinedButton(
                      key: const Key('company'),
                      onPressed:
                          _busy ||
                              !v.sessionVerified ||
                              v.currentDeviceId.isEmpty
                          ? null
                          : () => Navigator.of(context).push(
                              MaterialPageRoute<void>(
                                builder: (_) => CompanyScreen(client: _client!),
                              ),
                            ),
                      child: Text(l.companyTitle),
                    ),
                    OutlinedButton(
                      key: const Key('manage-passkeys'),
                      onPressed: _busy || !v.sessionVerified
                          ? null
                          : () => Navigator.of(context).push(
                              MaterialPageRoute<void>(
                                builder: (_) =>
                                    PasskeysScreen(client: _client!),
                              ),
                            ),
                      child: Text(l.passkeysTitle),
                    ),
                    OutlinedButton(
                      key: const Key('register-passkey'),
                      onPressed: _busy || !v.sessionVerified
                          ? null
                          : () => _startPasskey(true),
                      child: Text(l.registerPasskey),
                    ),
                    OutlinedButton(
                      key: const Key('refresh'),
                      onPressed: _busy
                          ? null
                          : () => _run(() => _client!.refresh(), prepare: true),
                      child: Text(l.refresh),
                    ),
                    FilledButton(
                      key: const Key('logout'),
                      onPressed: _busy
                          ? null
                          : () => _run(() => _client!.logout()),
                      child: Text(l.logout),
                    ),
                  ] else if ({
                    'github',
                    'passkey',
                    'passkey_registration',
                  }.contains(v.flow)) ...[
                    Text(v.flow == 'github' ? l.githubHelp : l.passkeyHelp),
                    const SizedBox(height: OpenNetworkSpacing.four),
                    SelectableText(
                      v.verificationCode,
                      semanticsLabel: v.verificationCode,
                      style: Theme.of(context).textTheme.headlineLarge,
                    ),
                    Text(l.validFor(v.expiresInSeconds)),
                    const SizedBox(height: OpenNetworkSpacing.four),
                    FilledButton(
                      onPressed: _busy ? null : _openGithub,
                      child: Text(
                        v.flow == 'github' ? l.openGithub : l.openPasskey,
                      ),
                    ),
                    if (_browserOpened) Text(l.awaitApproval),
                    TextButton(onPressed: _cancel, child: Text(l.cancel)),
                  ] else ...[
                    if (v.invitationPending) Text(l.invitationVerify),
                    if (!v.invitationPending && v.flow.isEmpty)
                      TextButton(
                        onPressed: _busy ? null : _importInvitation,
                        child: Text(l.invitationImport),
                      ),
                    TextField(
                      key: const Key('email'),
                      controller: _email,
                      enabled:
                          !_busy && (v.emailAvailable || v.passkeyAvailable),
                      keyboardType: TextInputType.emailAddress,
                      autocorrect: false,
                      autofillHints: const [AutofillHints.email],
                      decoration: InputDecoration(labelText: l.email),
                    ),
                    const SizedBox(height: OpenNetworkSpacing.four),
                    if (v.passkeyAvailable && !v.invitationPending)
                      Text(l.passkeyAccountHint),
                    if (v.flow == 'email') ...[
                      Text(l.emailHint),
                      const SizedBox(height: OpenNetworkSpacing.four),
                      TextField(
                        key: const Key('otp'),
                        controller: _code,
                        enabled: !_busy,
                        keyboardType: TextInputType.number,
                        obscureText: true,
                        maxLength: 8,
                        inputFormatters: [
                          FilteringTextInputFormatter.digitsOnly,
                        ],
                        autofillHints: const [AutofillHints.oneTimeCode],
                        decoration: InputDecoration(labelText: l.code),
                        onSubmitted: (_) {
                          if (!_busy) {
                            _run(() => _client!.verifyEmail(code: _code.text));
                          }
                        },
                      ),
                      FilledButton(
                        key: const Key('verify'),
                        onPressed: _busy
                            ? null
                            : () => _run(
                                () => _client!.verifyEmail(code: _code.text),
                              ),
                        child: Text(l.verify),
                      ),
                      Text(l.validFor(v.expiresInSeconds)),
                      if (v.retryAfterSeconds > 0)
                        Text(l.retryIn(v.retryAfterSeconds)),
                    ],
                    FilledButton(
                      key: const Key('send-code'),
                      onPressed:
                          _busy || !v.emailAvailable || v.retryAfterSeconds > 0
                          ? null
                          : () => _run(
                              () => _client!.requestEmail(
                                email: _email.text.trim(),
                                locale: _locale,
                              ),
                            ),
                      child: Text(v.flow == 'email' ? l.resend : l.sendCode),
                    ),
                    if (!v.emailAvailable) Text(l.unavailable),
                    const SizedBox(height: OpenNetworkSpacing.four),
                    OutlinedButton(
                      key: const Key('github'),
                      onPressed:
                          _busy || v.invitationPending || !v.githubAvailable
                          ? null
                          : _startGithub,
                      child: Text(l.github),
                    ),
                    if (!v.githubAvailable) Text(l.unavailable),
                    OutlinedButton(
                      key: const Key('passkey'),
                      onPressed:
                          _busy || v.invitationPending || !v.passkeyAvailable
                          ? null
                          : () => _startPasskey(false),
                      child: Text(l.passkey),
                    ),
                    if (v.flow.isNotEmpty || v.invitationPending)
                      TextButton(onPressed: _cancel, child: Text(l.cancel)),
                  ],
                  if (_busy) ...[
                    const SizedBox(height: OpenNetworkSpacing.four),
                    Semantics(
                      label: l.working,
                      child: const LinearProgressIndicator(),
                    ),
                  ],
                  if (_error != null) ...[
                    const SizedBox(height: OpenNetworkSpacing.four),
                    Semantics(
                      liveRegion: true,
                      child: Text(
                        _message(l, _error!),
                        key: const Key('auth-error'),
                        style: TextStyle(
                          color: Theme.of(context).colorScheme.error,
                        ),
                      ),
                    ),
                  ],
                  const SizedBox(height: OpenNetworkSpacing.six),
                  if (_client != null && !kIsWeb) ...[
                    TextButton(
                      onPressed: _busy
                          ? null
                          : () {
                              _poll?.cancel();
                              widget.onClient?.call(null);
                              _client!.cancel();
                              _client!.dispose();
                              _client = null;
                              _browserOpened = false;
                              _code.clear();
                              setState(() {
                                _view = null;
                                _error = null;
                              });
                            },
                      child: Text(l.changeServer),
                    ),
                  ],
                  if (diagnostics.sinkInitialized) ...[
                    DropdownButton<native.DebugScope>(
                      isExpanded: true,
                      isDense: false,
                      itemHeight: null,
                      value: diagnostics.debugScope ?? _debugScope,
                      items: [
                        DropdownMenuItem(
                          value: native.DebugScope.http,
                          child: Text(l.diagnosticsHttp),
                        ),
                        DropdownMenuItem(
                          value: native.DebugScope.io,
                          child: Text(l.diagnosticsIo),
                        ),
                      ],
                      onChanged: diagnostics.debugEnabled
                          ? null
                          : (scope) {
                              if (scope != null) {
                                setState(() => _debugScope = scope);
                              }
                            },
                    ),
                    TextButton(
                      onPressed: () {
                        try {
                          native.configureDebug(
                            scope: _debugScope,
                            seconds: diagnostics.debugEnabled ? 0 : 300,
                          );
                        } catch (_) {
                          widget.diagnostics?.reportFailure(
                            native.FailureKind.nativeBridge,
                          );
                          setState(() => _error = 'diagnostics_unavailable');
                        }
                        setState(() {});
                      },
                      child: Text(
                        diagnostics.debugEnabled
                            ? l.diagnosticsOff
                            : l.diagnostics,
                      ),
                    ),
                  ],
                  Text(
                    diagnostics.sinkInitialized &&
                            widget.diagnostics?.reportingUnavailable != true
                        ? l.localDiagnosticsOnly
                        : l.diagnosticsUnavailable,
                    key: const Key('diagnostics-status'),
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                  if (diagnostics.sinkInitialized &&
                      ((diagnostics.droppedEvents ?? BigInt.zero) >
                              BigInt.zero ||
                          (diagnostics.writeFailures ?? BigInt.zero) >
                              BigInt.zero ||
                          (diagnostics.rejectedFieldsOrEvents ?? BigInt.zero) >
                              BigInt.zero))
                    Text(
                      l.diagnosticsDegraded(
                        '${diagnostics.rejectedFieldsOrEvents}',
                        '${diagnostics.droppedEvents}',
                        '${diagnostics.writeFailures}',
                        '${diagnostics.pendingEvents}',
                      ),
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                      ),
                    ),
                  Text(
                    kIsWeb ? l.browserPrivacy : l.privacy,
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                  const SizedBox(height: OpenNetworkSpacing.four),
                  TextButton(
                    onPressed: () => launchUrl(
                      Uri.parse('https://nddev.ai'),
                      mode: LaunchMode.externalApplication,
                    ),
                    child: const Text('NDDev OpenNetwork · nddev.ai'),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
