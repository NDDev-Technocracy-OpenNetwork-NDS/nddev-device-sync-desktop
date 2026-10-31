import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:nds_ui/nds_ui.dart';
import 'package:nds_native/nds_native.dart' as native;

import 'fixture_identity.dart' as identity;

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets('actual local modules and isolated native clipboard archive', (
    tester,
  ) async {
    final frameworkHandler = FlutterError.onError;
    await runNdsApp();
    final diagnosticHandler = FlutterError.onError;
    FlutterError.onError = (details) {
      diagnosticHandler?.call(details);
      frameworkHandler?.call(details);
    };
    addTearDown(() async {
      await tester.pumpWidget(const SizedBox.shrink());
      FlutterError.onError = frameworkHandler;
    });
    await tester.pumpAndSettle();
    const server = String.fromEnvironment('NDS_TEST_SERVER');
    const mailbox = String.fromEnvironment('NDS_TEST_MAILPIT');
    const owner = String.fromEnvironment('NDS_TEST_OWNER');
    expect(Uri.parse(server).host, '127.0.0.1');
    expect(owner.endsWith('.invalid'), isTrue);
    await tester.enterText(find.byKey(const Key('server-address')), server);
    await tapVisible(tester, find.byKey(const Key('connect')));
    await identity.waitFor(tester, find.byKey(const Key('email')));
    await tester.enterText(find.byKey(const Key('email')), owner);
    final prior = await identity.localMessageIds(mailbox);
    await tapVisible(tester, find.byKey(const Key('send-code')));
    await identity.waitFor(tester, find.byKey(const Key('otp')));
    await tester.enterText(
      find.byKey(const Key('otp')),
      await identity.readLocalCode(mailbox, owner, prior),
    );
    await tapVisible(tester, find.byKey(const Key('verify')));
    await identity.waitFor(tester, find.byKey(const Key('logout')));
    await tapVisible(tester, find.text('Local modules'));
    await tester.pumpAndSettle();
    await waitFor(tester, find.byKey(const Key('module-selector')));
    // ADR 0018: a new enrollment is fullauto. A supported system tool runs
    // without the module checklist and without a prompt.
    await waitFor(tester, find.text('Server policy, revision 1'));
    expect(
      tester
          .widget<SegmentedButton<String>>(
            find.byKey(const Key('device-control-mode')),
          )
          .selected,
      {'fullauto'},
    );
    await tapVisible(tester, find.byKey(const Key('read-system-info')));
    await waitFor(tester, find.text('CPU usage:'));
    expect(find.byKey(const Key('module-error')), findsNothing);
    // Narrowing to manual closes fullauto admission; the existing checklist applies.
    await setControl(tester, 'Manual', 'Server policy, revision 2');
    await tapVisible(tester, find.byKey(const Key('read-system-info')));
    await waitFor(tester, find.byKey(const Key('module-error')));
    expect(
      tester.widget<Text>(find.byKey(const Key('module-error'))).data,
      'Enable this module to use its features.',
    );
    await configure(tester, enabled: true, permissions: []);
    await tapVisible(tester, find.byKey(const Key('read-system-info')));
    await waitFor(tester, find.byKey(const Key('module-error')));
    expect(
      tester.widget<Text>(find.byKey(const Key('module-error'))).data,
      'Choose the required NDS permissions before using this feature.',
    );
    await configure(
      tester,
      enabled: true,
      permissions: ['read_only', 'local_network'],
    );
    await tester.ensureVisible(find.byKey(const Key('read-system-info')));
    await tapVisible(tester, find.byKey(const Key('read-system-info')));
    await waitFor(tester, find.text('CPU usage:'));
    expect(find.byKey(const Key('module-error')), findsNothing);

    await choose(tester, 'Clipboard history');
    await configure(
      tester,
      enabled: true,
      permissions: ['clipboard', 'local_network'],
    );
    await tapVisible(tester, find.text('Refresh'));
    await waitFor(tester, find.byTooltip('Read text'));
    await tapVisible(tester, find.byTooltip('Read text'));
    await waitFor(tester, find.text('Text preview, up to 16 KiB.'));
    const content = 'NDS isolated archive: Русский / English';
    expect(
      find.byWidgetPredicate(
        (widget) => widget is SelectableText && widget.data == content,
      ),
      findsOneWidget,
    );
    expect(find.byKey(const Key('module-error')), findsNothing);

    await choose(tester, 'Cleaner');
    await configure(
      tester,
      enabled: true,
      permissions: ['read_only', 'process_execution'],
    );
    await tapVisible(tester, find.text('Diagnostics'));
    await waitFor(tester, find.text('Errors:'));
    expect(find.byKey(const Key('module-error')), findsNothing);

    await choose(tester, 'Updates');
    await configure(
      tester,
      enabled: true,
      permissions: ['read_only', 'process_execution'],
    );
    await tapVisible(tester, find.text('Update plan'));
    await waitFor(tester, find.textContaining('Channel:'));
    expect(find.byKey(const Key('module-error')), findsNothing);

    await choose(tester, 'GitHub');
    await configure(
      tester,
      enabled: true,
      permissions: ['read_only', 'process_execution'],
    );
    const repository = String.fromEnvironment('NDS_TEST_REPOSITORY');
    expect(repository.startsWith('/tmp/nds-ui-providers.'), isTrue);
    await tester.enterText(
      find.byKey(const Key('repository-path')),
      repository,
    );
    await tapVisible(tester, find.text('Refresh'));
    await waitFor(tester, find.text('Result:'));
    await waitFor(tester, find.text('Available'));
    expect(find.byKey(const Key('module-error')), findsNothing);

    if (const bool.fromEnvironment('NDS_TEST_RDS_UNAVAILABLE')) {
      await choose(tester, 'Remote screen');
      expect(
        tester
            .widget<FilledButton>(find.widgetWithText(FilledButton, 'Refresh'))
            .onPressed,
        isNull,
      );
    }
    await choose(tester, 'System information');
    await configure(tester, enabled: false, permissions: []);
    await tester.ensureVisible(find.byKey(const Key('read-system-info')));
    await tapVisible(tester, find.byKey(const Key('read-system-info')));
    await waitFor(tester, find.byKey(const Key('module-error')));
    expect(
      tester.widget<Text>(find.byKey(const Key('module-error'))).data,
      'Enable this module to use its features.',
    );
    // Widening within the recent sign-in window restores fullauto; the
    // explicit disable stays stored for a later manual choice.
    await setControl(tester, 'Full auto', 'Server policy, revision 3');
    await tapVisible(tester, find.byKey(const Key('read-system-info')));
    await waitFor(tester, find.text('CPU usage:'));
    expect(find.byKey(const Key('module-error')), findsNothing);
    expect(
      tester
          .widget<SwitchListTile>(find.byKey(const Key('module-enabled')))
          .value,
      isFalse,
    );
    expect(native.clientDiagnostics().sinkInitialized, isTrue);
    final deadline = DateTime.now().add(const Duration(seconds: 5));
    while (native.clientDiagnostics().pendingEvents != BigInt.zero) {
      if (DateTime.now().isAfter(deadline)) {
        throw StateError('Local diagnostic output did not drain');
      }
      await Future<void>.delayed(const Duration(milliseconds: 25));
    }
    final diagnostics = native.clientDiagnostics();
    expect(diagnostics.writeFailures, BigInt.zero);
    expect(diagnostics.droppedEvents, BigInt.zero);
    expect(diagnostics.rejectedFieldsOrEvents, BigInt.zero);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  }, timeout: const Timeout(Duration(minutes: 3)));
}

Future<void> setControl(
  WidgetTester tester,
  String mode,
  String confirmation,
) async {
  final segment = find.descendant(
    of: find.byKey(const Key('device-control-mode')),
    matching: find.text(mode),
  );
  await tapVisible(tester, segment);
  await tapVisible(tester, find.byKey(const Key('device-control-save')));
  await waitFor(tester, find.text(confirmation));
  expect(find.byKey(const Key('device-control-error')), findsNothing);
}

Future<void> choose(WidgetTester tester, String label) async {
  await tester.ensureVisible(find.byKey(const Key('module-selector')));
  await tapVisible(tester, find.byKey(const Key('module-selector')));
  await tester.pumpAndSettle();
  await tapVisible(tester, find.text(label).last);
  await tester.pumpAndSettle();
}

Future<void> waitFor(WidgetTester tester, Finder finder) async {
  final deadline = DateTime.now().add(const Duration(seconds: 35));
  while (finder.evaluate().isEmpty) {
    if (DateTime.now().isAfter(deadline)) {
      final error = find.byWidgetPredicate(
        (widget) =>
            widget is Text &&
            {
              const Key('module-error'),
              const Key('module-metadata-error'),
              const Key('module-settings-error'),
            }.contains(widget.key),
      );
      final text = error.evaluate().isEmpty
          ? 'no displayed error'
          : tester.widget<Text>(error).data;
      final navigation = tester.widget<NavigationBar>(
        find.byType(NavigationBar),
      );
      final labels = tester
          .widgetList<Text>(find.byType(Text))
          .map((widget) => widget.data)
          .whereType<String>()
          .take(32)
          .join(' | ');
      throw StateError(
        'Native module UI did not reach $finder: $text; '
        'page=${navigation.selectedIndex}; visible labels: $labels',
      );
    }
    await tester.pump(const Duration(milliseconds: 100));
    await Future<void>.delayed(const Duration(milliseconds: 100));
  }
  await tester.pumpAndSettle();
}

Future<void> configure(
  WidgetTester tester, {
  required bool enabled,
  required List<String> permissions,
}) async {
  final toggle = find.byKey(const Key('module-enabled'));
  await tester.ensureVisible(toggle);
  if (tester.widget<SwitchListTile>(toggle).value != enabled) {
    await tapVisible(tester, toggle);
    await tester.pumpAndSettle();
  }
  for (final checkbox in find.byType(CheckboxListTile).evaluate().toList()) {
    final widget = checkbox.widget as CheckboxListTile;
    final name = (widget.key! as ValueKey<String>).value.replaceFirst(
      'module-permission-',
      '',
    );
    final target = find.byKey(widget.key!);
    if (widget.value != permissions.contains(name)) {
      await tester.ensureVisible(target);
      await tapVisible(tester, target);
      await tester.pumpAndSettle();
    }
  }
  final save = find.byKey(const Key('module-settings-save'));
  await tester.ensureVisible(save);
  await tapVisible(tester, save);
  await waitFor(tester, find.byKey(const Key('module-settings-saved')));
  await waitFor(
    tester,
    find.byWidgetPredicate(
      (widget) =>
          widget is SwitchListTile &&
          widget.key == const Key('module-enabled') &&
          widget.onChanged != null,
    ),
  );
  await tester.pumpAndSettle();
}

Future<void> tapVisible(WidgetTester tester, Finder finder) async {
  await tester.ensureVisible(finder);
  await tester.pumpAndSettle();
  expect(finder.hitTestable(), findsOneWidget);
  await tester.tap(finder);
  await tester.pumpAndSettle();
}
