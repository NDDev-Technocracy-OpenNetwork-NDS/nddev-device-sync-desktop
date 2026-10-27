// UI ordering proof only: no Rust library, OS adapter, authorization, browser,
// transport or physical shutdown is exercised. The actual ModulesScreen owns
// every lifecycle decision; fakes only record calls and return held Futures.
import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nddev_opennetwork_design_system/opennetwork_design_system.dart';
import 'package:nds_native/nds_native.dart' as native;
// The generated mock API is deliberately not added to the production barrel.
// ignore: implementation_imports
import 'package:nds_native/src/rust/frb_generated.dart'
    as generated
    show RustLibApi;
import 'package:nds_ui/l10n/app_localizations.dart';
import 'package:nds_ui/modules_screen.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  late _SupportsOnlyApi api;
  setUp(() {
    api = _SupportsOnlyApi();
    // Pinned FRB calls this initMock, not init/initMockApi. Its fake entrypoint
    // stores only this API; it loads no FFI and runs no Rust initializers.
    native.RustLib.initMock(api: api);
  });
  tearDown(() {
    try {
      expect(api.unexpected, isEmpty);
    } finally {
      native.RustLib.dispose();
    }
  });

  testWidgets(
    'stale discovery awaits its exact shutdown before disposal or new discovery',
    (tester) async {
      final fixture = _Fixture();
      try {
        final first = fixture.client('first');
        final old = fixture.agent(first, 'old');
        final next = fixture.client('next');
        final replacement = fixture.agent(next, 'replacement');
        await fixture.mount(tester, first);
        await _until(tester, () => first.discoverCalls == 1);

        fixture.select(next);
        await _frames(tester, 4);
        expect(next.discoverCalls, 0);
        first.releaseDiscovery(0);
        await _until(tester, () => old.shutdownCalls == 1);
        expect(old.disposeCalls, 0);
        expect(next.discoverCalls, 0);

        old.releaseShutdown();
        await _until(tester, () => next.discoverCalls == 1);
        expect(old.disposeCalls, 1);
        expect(
          fixture.events.indexOf('old.dispose'),
          lessThan(fixture.events.indexOf('next.discover.1')),
        );
        next.releaseDiscovery(0);
        await _until(tester, () => next.metadataCalls > 0);
        expect(replacement.disposeCalls, 0);
      } finally {
        await fixture.close(tester);
      }
    },
    timeout: const Timeout(Duration(seconds: 30)),
  );

  testWidgets(
    'cancel joins the held UI operation then awaits shutdown before replacement',
    (tester) async {
      final fixture = _Fixture();
      try {
        final client = fixture.client('client');
        final old = fixture.agent(client, 'old');
        fixture.agent(client, 'replacement');
        client.releaseDiscovery(0);
        await fixture.mount(tester, client);
        await _until(
          tester,
          () => find.byKey(const Key('module-selector')).evaluate().isNotEmpty,
        );
        await tester.tap(find.byKey(const Key('module-selector')));
        await tester.pump(const Duration(milliseconds: 300));
        await tester.tap(find.text('Browser automation').last);
        await tester.pump(const Duration(milliseconds: 300));
        final read = find.byKey(const Key('browser-status'));
        await tester.ensureVisible(read);
        await tester.tap(read);
        await _until(tester, () => old.statusCalls == 1);

        final cancel = find.widgetWithText(TextButton, 'Cancel request');
        await tester.ensureVisible(cancel);
        await tester.tap(cancel);
        await _frames(tester, 4);
        expect(old.closeCalls, greaterThan(0));
        expect(old.shutdownCalls, 0);
        expect(old.disposeCalls, 0);
        expect(client.discoverCalls, 1);

        old.releaseStatus();
        await _until(tester, () => old.shutdownCalls == 1);
        expect(old.disposeCalls, 0);
        expect(client.discoverCalls, 1);
        expect(
          fixture.events.indexOf('old.status.release'),
          lessThan(fixture.events.indexOf('old.shutdown')),
        );

        old.releaseShutdown();
        await _until(tester, () => client.discoverCalls == 2);
        expect(old.disposeCalls, 1);
        expect(
          fixture.events.indexOf('old.dispose'),
          lessThan(fixture.events.indexOf('client.discover.2')),
        );
        client.releaseDiscovery(1);
        await _until(tester, () => client.metadataCalls >= 2);
      } finally {
        await fixture.close(tester);
      }
    },
    timeout: const Timeout(Duration(seconds: 30)),
  );

  testWidgets(
    'failed shutdown stays retained through client updates and metadata refresh',
    (tester) async {
      final fixture = _Fixture();
      try {
        final first = fixture.client('first');
        final old = fixture.agent(first, 'old');
        final next = fixture.client('next');
        fixture.agent(next, 'next-agent');
        final last = fixture.client('last');
        fixture.agent(last, 'last-agent');
        first.releaseDiscovery(0);
        await fixture.mount(tester, first);
        await _until(tester, () => first.metadataCalls > 0);

        fixture.select(next);
        await _until(tester, () => old.shutdownCalls == 1);
        old.failShutdown();
        await _until(
          tester,
          () => find.byKey(const Key('module-error')).evaluate().isNotEmpty,
        );
        fixture.select(last);
        await _frames(tester, 4);
        fixture.select(next);
        await _frames(tester, 4);
        expect(old.shutdownCalls, 1);
        expect(old.disposeCalls, 0);
        expect(next.discoverCalls, 0);
        expect(last.discoverCalls, 0);
        expect(find.byKey(const Key('module-error')), findsOneWidget);

        final before = next.metadataCalls;
        final refresh = find.byKey(const Key('module-settings-refresh'));
        await tester.ensureVisible(refresh);
        await tester.tap(refresh);
        await _until(tester, () => next.metadataCalls > before);
        expect(old.shutdownCalls, 1);
        expect(old.disposeCalls, 0);
        expect(next.discoverCalls, 0);
        expect(last.discoverCalls, 0);
        expect(find.byKey(const Key('module-error')), findsOneWidget);
        final read = tester.widget<FilledButton>(
          find.byKey(const Key('read-system-info')),
        );
        expect(read.onPressed, isNull);
        // A HOLD closes admission. A busy spinner is not evidence of active
        // native work, successful shutdown or an automatically retried drain.
      } finally {
        await fixture.close(tester);
      }
    },
    timeout: const Timeout(Duration(seconds: 30)),
  );
}

Future<void> _frames(WidgetTester tester, int count) async {
  for (var frame = 0; frame < count; frame++) {
    await tester.pump(const Duration(milliseconds: 1));
  }
}

Future<void> _until(WidgetTester tester, bool Function() ready) async {
  for (var frame = 0; frame < 32; frame++) {
    await tester.pump(const Duration(milliseconds: 1));
    if (ready()) return;
  }
  fail('UI ordering fixture did not reach the requested boundary in 32 frames');
}

class _SupportsOnlyApi implements generated.RustLibApi {
  final unexpected = <String>[];

  @override
  bool crateApiModulesSupportsLocalAgent() => true;

  @override
  dynamic noSuchMethod(Invocation invocation) {
    final member = invocation.memberName.toString();
    unexpected.add(member);
    throw TestFailure('Unexpected generated mock API call: $member');
  }
}

class _Fixture {
  final events = <String>[];
  final unexpected = <String>[];
  final clients = <_Client>[];
  final agents = <_Agent>[];
  ValueNotifier<native.NdsClient?>? _selected;

  _Client client(String label) {
    final value = _Client(label, events, unexpected);
    clients.add(value);
    return value;
  }

  _Agent agent(_Client owner, String label) {
    final value = _Agent(label, owner, events, unexpected);
    agents.add(value);
    owner.discoveries.add(_Discovery(value));
    return value;
  }

  Future<void> mount(WidgetTester tester, _Client client) async {
    tester.view.physicalSize = const Size(1200, 1600);
    tester.view.devicePixelRatio = 1;
    final selected = ValueNotifier<native.NdsClient?>(client);
    _selected = selected;
    await tester.pumpWidget(
      MaterialApp(
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: OpenNetworkTheme.dark(width: 1200),
        home: ValueListenableBuilder<native.NdsClient?>(
          valueListenable: selected,
          builder: (_, value, _) => ModulesScreen(
            key: const ValueKey('ordering-modules-screen'),
            client: value,
            onLocale: (_) {},
          ),
        ),
      ),
    );
  }

  void select(_Client client) => _selected!.value = client;

  Future<void> close(WidgetTester tester) async {
    // Settle every declared gate, including gates not reached because an
    // earlier assertion failed. Completing values cannot create an unhandled
    // error on a Future the screen never consumed.
    for (final client in clients) {
      for (final discovery in client.discoveries) {
        discovery.release();
      }
    }
    for (final agent in agents) {
      agent.releaseStatus();
      agent.releaseShutdown();
    }
    try {
      await _frames(tester, 16);
    } finally {
      try {
        await tester.pumpWidget(const SizedBox.shrink());
        await _frames(tester, 16);
      } finally {
        _selected?.dispose();
        tester.view.resetPhysicalSize();
        tester.view.resetDevicePixelRatio();
      }
    }
    for (final client in clients) {
      expect(
        client.discoveries.every((value) => value.completion.isCompleted),
        isTrue,
      );
    }
    for (final agent in agents) {
      expect(agent._status.isCompleted && agent._shutdown.isCompleted, isTrue);
    }
    expect(unexpected, isEmpty);
    expect(tester.takeException(), isNull);
  }
}

class _Discovery {
  _Discovery(this.agent);
  final _Agent agent;
  final completion = Completer<native.LocalAgent>();

  void release() {
    if (!completion.isCompleted) completion.complete(agent);
  }
}

class _Client implements native.NdsClient {
  _Client(this.label, this.events, this.unexpected);
  final String label;
  final List<String> events;
  final List<String> unexpected;
  final discoveries = <_Discovery>[];
  int discoverCalls = 0, metadataCalls = 0;

  void releaseDiscovery(int index) => discoveries[index].release();

  @override
  native.View snapshot() => native.View(
    invitationPending: false,
    devices: const [],
    currentDeviceId: 'fixture-device-$label',
    devicesCheckedAt: 0,
    emailAvailable: false,
    githubAvailable: false,
    passkeyAvailable: false,
    credentialStoreReady: true,
    serverVersion: 'fixture',
    authenticated: true,
    sessionVerified: true,
    offlineAuthorized: false,
    offlineCacheReady: false,
    logoutPending: false,
    credentialRevalidationRequired: false,
    sessionCheckedAt: 0,
    userId: 'fixture-user-$label',
    tenantId: 'fixture-tenant',
    authMethod: 'fixture',
    expiresAt: '',
    flow: '',
    authorizationUrl: '',
    verificationCode: '',
    expiresInSeconds: 0,
    retryAfterSeconds: 0,
  );

  @override
  Future<native.LocalAgent> discoverLocalAgent() {
    final index = discoverCalls++;
    events.add('$label.discover.$discoverCalls');
    if (index >= discoveries.length) {
      unexpected.add('$label.extra-discovery');
      throw TestFailure('Unexpected unplanned discovery in UI fixture');
    }
    return discoveries[index].completion.future;
  }

  @override
  Future<List<native.DeviceModuleView>> deviceModules() async {
    metadataCalls++;
    // Presentation data only, not a permission/admission grant. Rust/Core are
    // absent from these tests and their authorization cannot be inferred here.
    return [
      for (final id in ['sysinfo', 'browser-use'])
        native.DeviceModuleView(
          id: id,
          version: 'fixture',
          supported: true,
          installed: true,
          localToolSupported: true,
          localToolInstalled: true,
          enabled: true,
          permissions: const [],
          requiredPermissions: const [],
          optionalPermissions: const [],
          readiness: 'not_checked',
          revision: BigInt.one,
          conflicted: false,
        ),
    ];
  }

  @override
  dynamic noSuchMethod(Invocation invocation) {
    final member = invocation.memberName.toString();
    unexpected.add('$label.$member');
    throw TestFailure('Unexpected client fixture call: $member');
  }
}

class _Agent implements native.LocalAgent {
  _Agent(this.label, this.owner, this.events, this.unexpected);
  final String label;
  final _Client owner;
  final List<String> events;
  final List<String> unexpected;
  final _shutdown = Completer<bool>();
  final _status = Completer<native.BrowserStatus>();
  int closeCalls = 0, shutdownCalls = 0, disposeCalls = 0, statusCalls = 0;

  void releaseShutdown() {
    if (!_shutdown.isCompleted) {
      events.add('$label.shutdown.release');
      _shutdown.complete(true);
    }
  }

  void failShutdown() {
    if (!_shutdown.isCompleted) _shutdown.complete(false);
  }

  void releaseStatus() {
    if (!_status.isCompleted) {
      events.add('$label.status.release');
      _status.complete(
        native.BrowserStatus(
          enabled: true,
          browser: 'fixture',
          extension_: 'fixture',
          sessions: BigInt.zero,
        ),
      );
    }
  }

  @override
  Future<native.BrowserStatus> browserStatus({
    required native.NdsClient client,
  }) {
    expect(identical(client, owner), isTrue);
    statusCalls++;
    events.add('$label.status');
    return _status.future;
  }

  @override
  void close() {
    closeCalls++;
    events.add('$label.close');
    // Cancellation intent deliberately does not complete either held Future.
  }

  @override
  Future<void> shutdown() async {
    shutdownCalls++;
    events.add('$label.shutdown');
    if (!await _shutdown.future) throw StateError('fixture shutdown failure');
  }

  @override
  void dispose() {
    disposeCalls++;
    events.add('$label.dispose');
    // This fake has no native Arc, wire pointer, finalizer or process.
  }

  @override
  bool get isDisposed => disposeCalls > 0;

  @override
  dynamic noSuchMethod(Invocation invocation) {
    final member = invocation.memberName.toString();
    unexpected.add('$label.$member');
    throw TestFailure('Unexpected agent fixture call: $member');
  }
}
