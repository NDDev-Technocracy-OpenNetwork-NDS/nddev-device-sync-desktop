// Presentation ordering only. The real CompanyScreen owns the transitions;
// these generated-interface fakes supply values and held Futures, never grants.
import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nddev_opennetwork_design_system/opennetwork_design_system.dart';
import 'package:nds_native/nds_native.dart' as native;
// Keep the generated mock entrypoint out of the public application barrel.
// ignore: implementation_imports
import 'package:nds_native/src/rust/frb_generated.dart'
    as generated
    show RustLibApi;
import 'package:nds_ui/activity.dart';
import 'package:nds_ui/company_screen.dart';
import 'package:nds_ui/l10n/app_localizations.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  late _Api api;
  setUp(() {
    api = _Api();
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
    'personal starts unselected and old organization data cannot reach a replacement client',
    (tester) async {
      final fixture = _Fixture();
      try {
        await fixture.mount(tester);
        expect(fixture.client.selections, isEmpty);
        expect(find.text('only-a@example.invalid'), findsNothing);
        await fixture.select(tester, 'org-a');
        expect(fixture.client.selections, [('org-a', BigInt.from(7))]);
        expect(find.text('only-a@example.invalid'), findsOneWidget);

        await fixture.select(tester, 'org-b');
        expect(fixture.client.selections.last, ('org-b', BigInt.from(19)));
        expect(find.text('only-a@example.invalid'), findsNothing);
        expect(fixture.b.memberCalls, 0);
        expect(fixture.b.invitationCalls, 0);
        expect(fixture.a.closeCalls, greaterThan(0));

        // Select a fresh original-space handle, then hold its real UI read.
        fixture.client.nextA = fixture.a2;
        await fixture.select(tester, 'org-a');
        final held = fixture.holdOverview(fixture.a2);
        await tester.ensureVisible(find.text('Revalidate session'));
        await tester.tap(find.text('Revalidate session'));
        await _until(tester, () => fixture.a2.overviewCalls == 2);
        fixture.selected.value = fixture.replacement;
        await _frames(tester, 4);
        expect(fixture.a2.closeCalls, greaterThan(0));
        expect(fixture.a2.disposeCalls, 0);
        expect(find.text('only-a@example.invalid'), findsNothing);
        expect(find.byType(CompanyScreen), findsOneWidget);
        expect(fixture.replacement.collectionCalls, 0);

        held.complete(fixture.a2.view);
        await _until(
          tester,
          () => find.byType(CompanyScreen).evaluate().isEmpty,
        );
        expect(fixture.a2.disposeCalls, 1);
        expect(fixture.replacement.collectionCalls, 0);
        expect(find.byKey(const Key('open-organizations')), findsOneWidget);
      } finally {
        await fixture.finish(tester);
      }
    },
    timeout: const Timeout(Duration(seconds: 30)),
  );

  for (final sessionLoss in [false, true]) {
    testWidgets(
      '${sessionLoss ? 'session loss' : 'client replacement'} removes its owned confirmation without sending a command',
      (tester) async {
        final fixture = _Fixture();
        try {
          await fixture.mount(tester);
          await fixture.select(tester, 'org-a');
          final menu = find.byType(PopupMenuButton<String>);
          await tester.ensureVisible(menu);
          await tester.tap(menu);
          final administrator = find.widgetWithText(
            PopupMenuItem<String>,
            'Administrator',
          );
          await _until(
            tester,
            () => administrator.hitTestable().evaluate().isNotEmpty,
          );
          await tester.tap(administrator.hitTestable());
          await _until(
            tester,
            () => find.byType(AlertDialog).evaluate().isNotEmpty,
          );
          expect(find.byType(AlertDialog), findsOneWidget);
          expect(find.textContaining('only-a@example.invalid'), findsWidgets);

          if (sessionLoss) {
            fixture.client.authenticated = false;
            await tester.pump(const Duration(seconds: 1));
          } else {
            fixture.selected.value = fixture.replacement;
          }
          await _frames(tester, 4);
          expect(find.byType(AlertDialog), findsNothing);
          expect(find.textContaining('only-a@example.invalid'), findsNothing);
          await _until(
            tester,
            () => find.byType(CompanyScreen).evaluate().isEmpty,
          );
          expect(fixture.command.executeCalls, 0);
          expect(fixture.a.inviteCalls, 0);
          expect(fixture.replacement.collectionCalls, 0);
        } finally {
          await fixture.finish(tester);
        }
      },
      timeout: const Timeout(Duration(seconds: 30)),
    );
  }

  testWidgets(
    'normal back closes admission and waits for the submitted command',
    (tester) async {
      final fixture = _Fixture();
      try {
        await fixture.mount(tester);
        await fixture.select(tester, 'org-a');
        await fixture.invite(tester);
        expect(fixture.command.executeCalls, 1);
        await tester.pageBack();
        await _frames(tester, 4);
        expect(fixture.a.closeCalls, greaterThan(0));
        expect(fixture.a.disposeCalls, 0);
        expect(fixture.command.disposeCalls, 0);
        expect(find.byType(CompanyScreen), findsOneWidget);

        fixture.command.release(0);
        await _until(
          tester,
          () => find.byType(CompanyScreen).evaluate().isEmpty,
        );
        expect(fixture.command.executeCalls, 1);
        expect(fixture.a.inviteCalls, 1);
        expect(fixture.command.disposeCalls, 1);
        expect(fixture.a.disposeCalls, 1);
        expect(
          find.textContaining('server confirmed the change'),
          findsNothing,
        );
      } finally {
        await fixture.finish(tester);
      }
    },
    timeout: const Timeout(Duration(seconds: 30)),
  );

  testWidgets(
    'an unknown reply retains one command and refresh never resubmits it',
    (tester) async {
      final fixture = _Fixture();
      try {
        await fixture.mount(tester);
        await fixture.select(tester, 'org-a');
        await fixture.invite(tester);
        fixture.command.release(0, success: false);
        await _until(
          tester,
          () => find.byKey(const Key('company-error')).evaluate().isNotEmpty,
        );
        expect(fixture.command.disposeCalls, 0);
        expect(fixture.command.executeCalls, 1);
        final reads = fixture.client.collectionCalls;
        await tester.ensureVisible(find.text('Revalidate session'));
        await tester.tap(find.text('Revalidate session'));
        await _until(tester, () => fixture.client.collectionCalls > reads);
        await _frames(tester, 4);
        expect(fixture.command.executeCalls, 1);
        expect(fixture.a.inviteCalls, 1);
        expect(find.text('Retry the same command'), findsOneWidget);

        await tester.ensureVisible(find.text('Retry the same command'));
        await tester.tap(find.text('Retry the same command'));
        await _until(tester, () => fixture.command.executeCalls == 2);
        expect(fixture.a.inviteCalls, 1);
        fixture.command.release(1);
        await _until(tester, () => fixture.command.disposeCalls == 1);
        expect(find.text('Retry the same command'), findsNothing);
        expect(fixture.command.executeCalls, 2);
      } finally {
        await fixture.finish(tester);
      }
    },
    timeout: const Timeout(Duration(seconds: 30)),
  );
}

Future<void> _frames(WidgetTester tester, int count) async {
  for (var i = 0; i < count; i++) {
    await tester.pump(const Duration(milliseconds: 1));
  }
}

Future<void> _until(WidgetTester tester, bool Function() ready) async {
  for (var i = 0; i < 48; i++) {
    await tester.pump(const Duration(milliseconds: 10));
    if (ready()) return;
  }
  fail(
    'Organization fixture did not reach its requested boundary in 48 frames',
  );
}

class _Api implements generated.RustLibApi {
  final unexpected = <String>[];
  @override
  bool crateApiScreenScreenSupported() => false;
  @override
  dynamic noSuchMethod(Invocation invocation) {
    unexpected.add(invocation.memberName.toString());
    throw TestFailure('Unexpected generated API call');
  }
}

native.OrganizationOverviewView _overview(
  String id,
  native.OrganizationRoleView role,
  int revision,
) => native.OrganizationOverviewView(
  companyId: id,
  name: 'Shared name',
  revision: BigInt.one,
  member: native.OrganizationMemberView(
    userId: 'original-user',
    email: 'self@example.invalid',
    role: role,
    active: true,
    revision: BigInt.from(revision),
  ),
);

class _Fixture {
  final unexpected = <String>[];
  final command = _Command();
  late final a = _Organization(
    _overview('org-a', native.OrganizationRoleView.admin, 7),
    command,
    unexpected,
  );
  late final a2 = _Organization(a.view, command, unexpected);
  late final b = _Organization(
    _overview('org-b', native.OrganizationRoleView.user, 19),
    command,
    unexpected,
  );
  late final client = _Client([a.view, b.view], a, b, unexpected);
  late final replacement = _Client([a.view, b.view], a2, b, unexpected);
  late final selected = ValueNotifier<native.NdsClient>(client);
  late final activity = ApplicationActivity(nativeReady: false)..value = true;
  final holds = <Completer<native.OrganizationOverviewView>>[];

  Completer<native.OrganizationOverviewView> holdOverview(_Organization owner) {
    final held = Completer<native.OrganizationOverviewView>();
    holds.add(held);
    owner.held = held;
    return held;
  }

  Future<void> mount(WidgetTester tester) async {
    tester.view.physicalSize = const Size(1200, 1800);
    tester.view.devicePixelRatio = 1;
    await tester.pumpWidget(
      ApplicationActivityScope(
        activity: activity,
        child: MaterialApp(
          locale: const Locale('en'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: OpenNetworkTheme.dark(width: 1200),
          home: Builder(
            builder: (context) => Scaffold(
              body: FilledButton(
                key: const Key('open-organizations'),
                onPressed: () => Navigator.of(context).push<void>(
                  MaterialPageRoute<void>(
                    builder: (_) => ValueListenableBuilder<native.NdsClient>(
                      valueListenable: selected,
                      builder: (_, value, _) => CompanyScreen(
                        key: const Key('owned-organizations'),
                        client: value,
                      ),
                    ),
                  ),
                ),
                child: const Text('Open organizations'),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.byKey(const Key('open-organizations')));
    await tester.pump(const Duration(milliseconds: 350));
    await _until(tester, () => client.collectionCalls == 1);
  }

  Future<void> select(WidgetTester tester, String id) async {
    final selector = find.byWidgetPredicate(
      (w) => w is DropdownButtonFormField<String>,
    );
    await tester.ensureVisible(selector);
    await tester.tap(selector);
    await tester.pump(const Duration(milliseconds: 300));
    await tester.tap(find.textContaining(' · $id').last);
    await tester.pump(const Duration(milliseconds: 300));
    await _until(tester, () => client.selections.last.$1 == id);
  }

  Future<void> invite(WidgetTester tester) async {
    await tester.ensureVisible(find.byKey(const Key('company-email')));
    await tester.enterText(
      find.byKey(const Key('company-email')),
      'invited@example.invalid',
    );
    await tester.ensureVisible(find.byKey(const Key('company-invite')));
    await tester.tap(find.byKey(const Key('company-invite')));
    await _until(tester, () => command.executeCalls == 1);
  }

  Future<void> finish(WidgetTester tester) async {
    for (final held in holds) {
      if (!held.isCompleted) held.complete(a.view);
    }
    command.release(0);
    command.release(1);
    try {
      await _frames(tester, 16);
    } finally {
      try {
        await tester.pumpWidget(const SizedBox.shrink());
        await _frames(tester, 16);
      } finally {
        selected.dispose();
        activity.dispose();
        tester.view.resetPhysicalSize();
        tester.view.resetDevicePixelRatio();
      }
    }
    expect(holds.every((h) => h.isCompleted), isTrue);
    expect(command.completions.every((c) => c.isCompleted), isTrue);
    expect(unexpected, isEmpty);
  }
}

class _Client implements native.NdsClient {
  _Client(this.choices, this.nextA, this.b, this.unexpected);
  final List<native.OrganizationOverviewView> choices;
  _Organization nextA;
  final _Organization b;
  final List<String> unexpected;
  int collectionCalls = 0;
  bool authenticated = true;
  final selections = <(String, BigInt)>[];
  @override
  native.View snapshot() => native.View(
    invitationPending: false,
    devices: [],
    currentDeviceId: 'original-device',
    devicesCheckedAt: 0,
    emailAvailable: true,
    githubAvailable: false,
    passkeyAvailable: false,
    credentialStoreReady: true,
    serverVersion: 'fixture',
    authenticated: authenticated,
    sessionVerified: true,
    offlineAuthorized: false,
    offlineCacheReady: false,
    logoutPending: false,
    credentialRevalidationRequired: false,
    sessionCheckedAt: 0,
    userId: 'original-user',
    tenantId: 'org-a',
    authMethod: 'email',
    expiresAt: '',
    flow: '',
    authorizationUrl: '',
    verificationCode: '',
    expiresInSeconds: 0,
    retryAfterSeconds: 0,
  );
  @override
  Future<List<native.OrganizationOverviewView>> organizations() async {
    collectionCalls++;
    return choices;
  }

  @override
  Future<native.LocalOrganization> selectOrganization({
    required String company,
    required BigInt membershipRevision,
  }) async {
    selections.add((company, membershipRevision));
    return company == 'org-a' ? nextA : b;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) {
    unexpected.add(invocation.memberName.toString());
    throw TestFailure('Unexpected personal client call');
  }
}

class _Organization implements native.LocalOrganization {
  _Organization(this.view, this.command, this.unexpected);
  final native.OrganizationOverviewView view;
  final _Command command;
  final List<String> unexpected;
  bool current = true;
  int closeCalls = 0,
      disposeCalls = 0,
      overviewCalls = 0,
      memberCalls = 0,
      invitationCalls = 0,
      inviteCalls = 0;
  Completer<native.OrganizationOverviewView>? held;
  @override
  bool isCurrent() => current;
  @override
  Future<native.OrganizationOverviewView> overview() async {
    overviewCalls++;
    return held == null ? view : await held!.future;
  }

  @override
  Future<native.OrganizationMemberPageView> members({String? after}) async {
    memberCalls++;
    return native.OrganizationMemberPageView(
      members: [
        native.OrganizationMemberView(
          userId: 'only-a',
          email: 'only-a@example.invalid',
          role: native.OrganizationRoleView.user,
          active: true,
          revision: BigInt.one,
        ),
      ],
    );
  }

  @override
  Future<native.OrganizationInvitationPageView> invitations({
    String? after,
  }) async {
    invitationCalls++;
    return const native.OrganizationInvitationPageView(invitations: []);
  }

  @override
  Future<native.CompanyCommand> invite({
    required String email,
    required native.OrganizationRoleView role,
  }) async {
    inviteCalls++;
    return command;
  }

  @override
  void close() {
    closeCalls++;
    current = false;
  }

  @override
  void dispose() {
    disposeCalls++;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) {
    unexpected.add(invocation.memberName.toString());
    throw TestFailure('Unexpected selected organization call');
  }
}

class _Command implements native.CompanyCommand {
  final completions = [Completer<bool>(), Completer<bool>()];
  int executeCalls = 0, disposeCalls = 0;
  @override
  bool isCurrent() => true;
  void release(int index, {bool success = true}) {
    if (!completions[index].isCompleted) completions[index].complete(success);
  }

  @override
  Future<void> execute() async {
    final index = executeCalls++;
    if (index >= completions.length) {
      throw TestFailure('Unexpected command replay');
    }
    if (!await completions[index].future) {
      throw StateError('synthetic unknown reply');
    }
  }

  @override
  void dispose() {
    disposeCalls++;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw TestFailure('Unexpected command API call');
}
