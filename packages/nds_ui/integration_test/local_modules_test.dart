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
    await tester.tap(find.byKey(const Key('connect')));
    await identity.waitFor(tester, find.byKey(const Key('email')));
    await tester.enterText(find.byKey(const Key('email')), owner);
    final prior = await identity.localMessageIds(mailbox);
    await tester.tap(find.byKey(const Key('send-code')));
    await identity.waitFor(tester, find.byKey(const Key('otp')));
    await tester.enterText(
      find.byKey(const Key('otp')),
      await identity.readLocalCode(mailbox, owner, prior),
    );
    await tester.tap(find.byKey(const Key('verify')));
    await identity.waitFor(tester, find.byKey(const Key('logout')));
    await tester.tap(find.text('Local modules'));
    await tester.pumpAndSettle();
    await waitFor(tester, find.byKey(const Key('module-selector')));
    await tester.tap(find.byKey(const Key('read-system-info')));
    await waitFor(tester, find.byKey(const Key('module-error')));
    expect(
      tester.widget<Text>(find.byKey(const Key('module-error'))).data,
      'Enable this module to use its features.',
    );
    await configure(tester, enabled: true, permissions: []);
    await tester.tap(find.byKey(const Key('read-system-info')));
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
    await tester.tap(find.byKey(const Key('read-system-info')));
    await waitFor(tester, find.text('CPU usage:'));
    expect(find.byKey(const Key('module-error')), findsNothing);

    await choose(tester, 'Clipboard history');
    await configure(
      tester,
      enabled: true,
      permissions: ['clipboard', 'local_network'],
    );
    await tester.tap(find.text('Refresh'));
    await waitFor(tester, find.byTooltip('Read text'));
    await tester.tap(find.byTooltip('Read text'));
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
    await tester.tap(find.text('Diagnostics'));
    await waitFor(tester, find.text('Errors:'));
    expect(find.byKey(const Key('module-error')), findsNothing);

    await choose(tester, 'Updates');
    await configure(
      tester,
      enabled: true,
      permissions: ['read_only', 'process_execution'],
    );
    await tester.tap(find.text('Update plan'));
    await waitFor(tester, find.textContaining('Channel:'));
    expect(find.byKey(const Key('module-error')), findsNothing);

    await choose(tester, 'GDS');
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
    await tester.tap(find.text('Refresh'));
    await waitFor(tester, find.text('Result:'));
    expect(find.byKey(const Key('module-error')), findsNothing);

    if (const bool.fromEnvironment('NDS_TEST_RDS_UNAVAILABLE')) {
      await choose(tester, 'RDS');
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
    await tester.tap(find.byKey(const Key('read-system-info')));
    await waitFor(tester, find.byKey(const Key('module-error')));
    expect(
      tester.widget<Text>(find.byKey(const Key('module-error'))).data,
      'Enable this module to use its features.',
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
  });
}

Future<void> choose(WidgetTester tester, String label) async {
  await tester.ensureVisible(find.byKey(const Key('module-selector')));
  await tester.tap(find.byKey(const Key('module-selector')));
  await tester.pumpAndSettle();
  await tester.tap(find.text(label).last);
  await tester.pumpAndSettle();
}

Future<void> waitFor(WidgetTester tester, Finder finder) async {
  final deadline = DateTime.now().add(const Duration(seconds: 35));
  while (finder.evaluate().isEmpty) {
    if (DateTime.now().isAfter(deadline)) {
      final error = find.byKey(const Key('module-error'));
      final text = error.evaluate().isEmpty
          ? 'no displayed error'
          : tester.widget<Text>(error).data;
      throw StateError(
        'Native module UI did not reach its expected state: $text',
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
    await tester.tap(toggle);
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
      await tester.tap(target);
      await tester.pumpAndSettle();
    }
  }
  final save = find.byKey(const Key('module-settings-save'));
  await tester.ensureVisible(save);
  await tester.tap(save);
  await waitFor(tester, find.byKey(const Key('module-settings-saved')));
  await tester.pumpAndSettle();
}
