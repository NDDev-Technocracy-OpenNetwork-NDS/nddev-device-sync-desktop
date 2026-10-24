import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:nds_ui/nds_ui.dart';
import 'package:nds_ui/module_settings.dart';
import 'package:nds_ui/state_screen.dart';

import 'fixture_identity.dart';
import 'local_modules_test.dart' as modules;
import 'device_modules_test.dart' as device;

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  const server = String.fromEnvironment('NDS_TEST_SERVER');
  const mailbox = String.fromEnvironment('NDS_TEST_MAILPIT');
  const owner = String.fromEnvironment('NDS_TEST_OWNER');
  const vault = bool.fromEnvironment('NDS_TEST_VAULT');
  const catalogSource = bool.fromEnvironment('NDS_TEST_CATALOG_SOURCE');
  const company = bool.fromEnvironment('NDS_TEST_COMPANY');
  const screenPeer = String.fromEnvironment('NDS_TEST_SCREEN_PEER');
  const screenHandoff = String.fromEnvironment('NDS_TEST_SCREEN_HANDOFF');
  const phase = String.fromEnvironment(
    'NDS_TEST_PHASE',
    defaultValue: 'complete',
  );

  testWidgets(
    catalogSource
        ? 'actual saved GitHub choice, background catalog and encrypted convergence'
        : 'actual identity, device enrollment, native restore and revocation',
    (tester) async {
      expect(Uri.parse(server).host, '127.0.0.1');
      expect(Uri.parse(mailbox).host, '127.0.0.1');
      expect(owner.endsWith('.invalid') || owner.endsWith('.test'), isTrue);
      final frameworkHandler = FlutterError.onError;
      await runNdsApp();
      final diagnosticHandler = FlutterError.onError;
      // Production reports redacted classes. The acceptance harness must also
      // receive unexpected framework failures rather than silently swallow them.
      FlutterError.onError = (details) {
        diagnosticHandler?.call(details);
        frameworkHandler?.call(details);
      };
      addTearDown(() async {
        await tester.pumpWidget(const SizedBox.shrink());
        FlutterError.onError = frameworkHandler;
      });
      await tester.pumpAndSettle();
      await tester.enterText(find.byKey(const Key('server-address')), server);
      await tester.tap(find.byKey(const Key('connect')));
      if (phase == 'restore') {
        await waitFor(tester, find.byKey(const Key('logout')));
        expect(find.text('This installation'), findsOneWidget);
        if (vault) {
          await tester.ensureVisible(find.byKey(const Key('open-vault')));
          await tester.pumpAndSettle();
          await tester.tap(find.byKey(const Key('open-vault')));
          await waitFor(tester, find.text('UI encrypted record'));
          expect(find.byKey(const Key('vault-create')), findsNothing);
          expect(find.text('PRIVATE_UI_VAULT_VALUE'), findsNothing);
          await tester.pageBack();
          await tester.pumpAndSettle();
        }
        await tester.ensureVisible(find.byKey(const Key('logout')));
        await tester.tap(find.byKey(const Key('logout')));
        await waitFor(tester, find.byKey(const Key('email')));
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
        return;
      }
      // A failed acceptance may leave this owned loopback fixture signed in.
      // Revoke only its own session before repeating the full interaction.
      await tester.pumpAndSettle();
      if (find.byKey(const Key('logout')).evaluate().isNotEmpty) {
        await tester.ensureVisible(find.byKey(const Key('logout')));
        await tester.tap(find.byKey(const Key('logout')));
      }
      await waitFor(tester, find.byKey(const Key('email')));
      await tester.enterText(find.byKey(const Key('email')), owner);
      final priorMessages = await localMessageIds(mailbox);
      await tester.tap(find.byKey(const Key('send-code')));
      await waitFor(tester, find.byKey(const Key('otp')));

      final code = await readLocalCode(mailbox, owner, priorMessages);
      await tester.enterText(find.byKey(const Key('otp')), code);
      await tester.tap(find.byKey(const Key('verify')));
      await waitFor(tester, find.byKey(const Key('logout')));
      expect(find.byKey(const Key('otp')), findsNothing);
      expect(tester.takeException(), isNull);
      await waitFor(tester, find.text('This installation'));
      Finder currentCard() => find.ancestor(
        of: find.text('This installation'),
        matching: find.byType(Card),
      );
      expect(
        find.descendant(
          of: currentCard(),
          matching: find.text('Acceptance device'),
        ),
        findsOneWidget,
      );
      expect(find.byKey(const Key('enroll-device')), findsNothing);
      if (screenPeer.isNotEmpty) {
        expect(Platform.isLinux, isTrue);
        expect(screenPeer.endsWith('@example.invalid'), isTrue);
        expect(Directory(screenHandoff).isAbsolute, isTrue);
        await modules.tapVisible(tester, find.text('Local modules'));
        await modules.waitFor(tester, find.byKey(const Key('module-selector')));
        await modules.choose(tester, 'RDS');
        await modules.configure(
          tester,
          enabled: true,
          permissions: ['read_only', 'local_network'],
        );
        await modules.tapVisible(tester, find.text('NDS session'));
        await writeScreenFact(screenHandoff, 'viewer-ui-ready.json', {
          'ready': true,
        });
        await waitScreenFact(tester, screenHandoff, 'host-device.json');
        await tester.ensureVisible(find.byKey(const Key('company')));
        await tester.tap(find.byKey(const Key('company')));
        await waitFor(tester, find.text(screenPeer));
        final member = find.ancestor(
          of: find.text(screenPeer),
          matching: find.byType(Card),
        );
        final menu = find.descendant(
          of: member,
          matching: find.byType(PopupMenuButton<String>),
        );
        await tester.ensureVisible(menu);
        await tester.tap(menu);
        await waitFor(tester, find.text('View screen').hitTestable());
        await tester.tap(find.text('View screen').hitTestable());
        await waitFor(tester, find.byKey(const Key('screen-targets')));
        await waitFor(tester, find.text('Owned screen host'));
        final target = find.ancestor(
          of: find.text('Owned screen host'),
          matching: find.byType(Card),
        );
        final open = find.descendant(
          of: target,
          matching: find.widgetWithText(TextButton, 'View screen'),
        );
        await tester.ensureVisible(open);
        await waitFor(
          tester,
          find.byWidgetPredicate(
            (w) =>
                w is TextButton &&
                w.onPressed != null &&
                w.child is Text &&
                (w.child! as Text).data == 'View screen',
          ),
        );
        await tester.tap(open);
        Finder image() => find.byWidgetPredicate(
          (widget) => widget is RawImage && widget.image != null,
        );
        await waitFor(tester, image());
        await writeScreenFact(screenHandoff, 'viewer-capture.json', {
          'capturing': true,
        });
        await waitScreenFact(tester, screenHandoff, 'host-painted.json');
        final deadline = DateTime.now().add(const Duration(seconds: 10));
        while (true) {
          final displayed = tester
              .widget<RawImage>(image().first)
              .image!
              .clone();
          int red = 0;
          final width = displayed.width, height = displayed.height;
          try {
            final data = await displayed.toByteData(
              format: ui.ImageByteFormat.rawRgba,
            );
            expect(data, isNotNull);
            final bytes = data!.buffer.asUint8List();
            for (var i = 0; i + 3 < bytes.length; i += 4) {
              if (bytes[i] > 150 && bytes[i + 1] < 90 && bytes[i + 2] < 90) {
                red++;
              }
            }
          } finally {
            displayed.dispose();
          }
          if (red > width * height ~/ 2) {
            await writeScreenFact(screenHandoff, 'viewer-frame.json', {
              'width': width,
              'height': height,
              'red_pixels': red,
              'flutter_rendered': true,
            });
            break;
          }
          expect(
            DateTime.now().isBefore(deadline),
            isTrue,
            reason: 'Real Flutter image did not reflect the owned source color',
          );
          await tester.pump(const Duration(milliseconds: 100));
          await Future<void>.delayed(const Duration(milliseconds: 100));
        }
        await waitScreenFact(tester, screenHandoff, 'demoted.json');
        final stopped = Stopwatch()..start();
        while (image().evaluate().isNotEmpty) {
          expect(
            stopped.elapsed < const Duration(seconds: 10),
            isTrue,
            reason: 'Flutter retained a revoked screen image',
          );
          await tester.pump(const Duration(milliseconds: 100));
          await Future<void>.delayed(const Duration(milliseconds: 100));
        }
        await writeScreenFact(screenHandoff, 'viewer-proof.json', {
          'stopped': true,
          'image_removed': true,
          'observed_stop_ms': stopped.elapsedMilliseconds,
        });
        await tester.pageBack();
        await waitFor(tester, find.byKey(const Key('screen-targets-error')));
        expect(find.text('Owned screen host'), findsNothing);
        await tester.pageBack();
        await waitFor(tester, find.text('User'));
        expect(find.byKey(const Key('company-email')), findsNothing);
        expect(tester.takeException(), isNull);
        await tester.pageBack();
        await tester.pumpAndSettle();
      }
      if (company && screenPeer.isEmpty) {
        await tester.ensureVisible(find.byKey(const Key('company')));
        await tester.tap(find.byKey(const Key('company')));
        await waitFor(tester, find.byKey(const Key('company-email')));
        final recipient =
            'ui-${DateTime.now().microsecondsSinceEpoch}@example.invalid';
        await tester.enterText(
          find.byKey(const Key('company-email')),
          recipient,
        );
        FocusManager.instance.primaryFocus?.unfocus();
        await tester.ensureVisible(find.byKey(const Key('company-invite')));
        await tester.tap(find.byKey(const Key('company-invite')));
        await waitFor(tester, find.text(recipient));
        expect(
          tester
              .widget<TextField>(find.byKey(const Key('company-email')))
              .controller!
              .text,
          isEmpty,
        );
        final card = find.ancestor(
          of: find.text(recipient),
          matching: find.byType(Card),
        );
        final revoke = find.descendant(
          of: card,
          matching: find.widgetWithText(TextButton, 'Revoke invitation'),
        );
        await tester.scrollUntilVisible(
          revoke,
          300,
          scrollable: find
              .descendant(
                of: find.byType(ListView).first,
                matching: find.byType(Scrollable),
              )
              .first,
        );
        await tester.pumpAndSettle();
        await tester.ensureVisible(revoke);
        await tester.pumpAndSettle();
        expect(revoke.hitTestable(), findsOneWidget);
        await tester.tap(revoke);
        await waitFor(
          tester,
          find.descendant(of: card, matching: find.textContaining('· Revoked')),
        );
        expect(tester.takeException(), isNull);
        await tester.pageBack();
        await tester.pumpAndSettle();
      }
      if (vault) {
        await tester.ensureVisible(find.byKey(const Key('open-vault')));
        await tester.tap(find.byKey(const Key('open-vault')));
        await waitFor(tester, find.byKey(const Key('vault-title')));
        final titleEditor = find.byKey(const Key('vault-title'));
        await tester.ensureVisible(titleEditor);
        await tester.pumpAndSettle();
        await tester.tap(titleEditor);
        await tester.pump();
        final focus = tester
            .widget<EditableText>(
              find.descendant(
                of: titleEditor,
                matching: find.byType(EditableText),
              ),
            )
            .focusNode;
        expect(focus.hasFocus, isTrue);
        // Cross multiple actual background snapshot ticks with the editor active.
        for (var tick = 0; tick < 25; tick++) {
          await tester.pump(const Duration(milliseconds: 100));
          await Future<void>.delayed(const Duration(milliseconds: 100));
          expect(
            focus.hasFocus,
            isTrue,
            reason: 'Background refresh must retain text-entry focus',
          );
        }
        await tester.enterText(
          find.byKey(const Key('vault-title')),
          'UI encrypted record',
        );
        await tester.enterText(
          find.byKey(const Key('vault-username')),
          'fixture-user',
        );
        await tester.enterText(
          find.byKey(const Key('vault-secret')),
          'PRIVATE_UI_VAULT_VALUE',
        );
        FocusManager.instance.primaryFocus?.unfocus();
        await tester.pumpAndSettle();
        await tester.ensureVisible(find.byKey(const Key('vault-save')));
        await tester.pumpAndSettle();
        await tester.tap(find.byKey(const Key('vault-save')));
        await waitFor(tester, find.text('UI encrypted record'));
        expect(
          tester
              .widget<TextField>(find.byKey(const Key('vault-secret')))
              .controller!
              .text,
          isEmpty,
        );
        expect(find.text('PRIVATE_UI_VAULT_VALUE'), findsNothing);
        expect(tester.takeException(), isNull);
        await tester.ensureVisible(find.byKey(const Key('open-synced-state')));
        await tester.pumpAndSettle();
        await tester.tap(find.byKey(const Key('open-synced-state')));
        await waitFor(tester, find.byKey(const Key('publish-device-state')));
        await tester.tap(find.byKey(const Key('publish-device-state')));
        await waitFor(tester, find.text('Email code'));
        final stateClient = tester
            .widget<StateScreen>(find.byType(StateScreen))
            .client;
        final ownDevice = find.byKey(
          ValueKey('state-${stateClient.snapshot().currentDeviceId}'),
        );
        await waitFor(tester, ownDevice);
        expect(
          find.descendant(
            of: ownDevice,
            matching: find.text('Acceptance device'),
          ),
          findsOneWidget,
        );
        expect(tester.takeException(), isNull);
        await tester.pageBack();
        await tester.pumpAndSettle();
        await tester.pageBack();
        await tester.pumpAndSettle();
        if (nativeDesktop()) {
          await tester.tap(find.text('Local modules'));
          await waitFor(
            tester,
            find.byWidgetPredicate(
              (widget) =>
                  widget is OutlinedButton &&
                  widget.key == const Key('publish-local-state') &&
                  widget.onPressed != null,
            ),
          );
          await tester.tap(find.byKey(const Key('publish-local-state')));
          await waitFor(
            tester,
            find.text('Local states queued for synchronization.'),
          );
          await tester.tap(find.text('NDS session'));
          await tester.pumpAndSettle();
          await tester.ensureVisible(find.byKey(const Key('open-vault')));
          await tester.pumpAndSettle();
          await tester.tap(find.byKey(const Key('open-vault')));
          await waitFor(tester, find.byKey(const Key('open-synced-state')));
          // The module action acknowledges its local queue. Actual convergence
          // is observed separately through the live Rust-owned Vault view.
          await waitFor(tester, find.text('Pending: 0 · Conflicts: 0'));
          await tester.ensureVisible(
            find.byKey(const Key('open-synced-state')),
          );
          await tester.pumpAndSettle();
          await tester.tap(find.byKey(const Key('open-synced-state')));
          await waitFor(tester, find.text('gds'));
          expect(find.text('rds'), findsOneWidget);
          await tester.pageBack();
          await tester.pumpAndSettle();
          await tester.pageBack();
          await tester.pumpAndSettle();
        }
      }
      if (catalogSource) {
        final picker = find.byType(DropdownButtonFormField<String>);
        final readyPicker = find.byWidgetPredicate(
          (widget) =>
              widget is DropdownButtonFormField<String> &&
              widget.onChanged != null,
        );
        for (var opening = 0; opening < 2; opening++) {
          await modules.tapVisible(
            tester,
            find.byKey(const Key('open-github-catalog')),
          );
          await modules.tapVisible(
            tester,
            find.byKey(const Key('catalog-mode-provider')),
          );
          await waitFor(tester, readyPicker);
          if (opening == 0) {
            expect(
              tester
                  .widget<DropdownButtonFormField<String>>(picker)
                  .initialValue,
              isNull,
            );
            await tester.tap(picker);
            await tester.pumpAndSettle();
            await tester.tap(find.text('UI encrypted record').last);
            await waitFor(
              tester,
              find.byKey(const Key('catalog-source-clear')),
            );
          }
          await waitFor(tester, readyPicker);
          expect(
            tester.widget<DropdownButtonFormField<String>>(picker).initialValue,
            isNotNull,
          );
          expect(find.text('PRIVATE_UI_VAULT_VALUE'), findsNothing);
          if (opening == 1) {
            await modules.tapVisible(
              tester,
              find.byKey(const Key('catalog-source-clear')),
            );
            await waitFor(tester, readyPicker);
            expect(
              tester
                  .widget<DropdownButtonFormField<String>>(picker)
                  .initialValue,
              isNull,
            );
            expect(find.byKey(const Key('catalog-source-clear')), findsNothing);
          }
          await tester.pageBack();
          await tester.pumpAndSettle();
          if (opening == 0) {
            await modules.tapVisible(tester, find.text('Local modules'));
            await modules.waitFor(
              tester,
              find.byKey(const Key('module-selector')),
            );
            await modules.choose(tester, 'GDS');
            await modules.configure(
              tester,
              enabled: true,
              permissions: ['read_only', 'credential_store', 'local_network'],
            );
            final client = tester
                .widget<ModuleSettings>(find.byType(ModuleSettings))
                .client;
            final deadline = DateTime.now().add(const Duration(seconds: 45));
            // No foreground provider action: the existing replica must collect
            // after this page has been closed, using the saved source/consent.
            while (true) {
              final observed = (await device.settled(client.deviceModules))
                  .singleWhere((module) => module.id == 'gds');
              if (observed.checkedAtMs != null &&
                  observed.errorClass == 'provider_unavailable') {
                break;
              }
              if (DateTime.now().isAfter(deadline)) {
                throw StateError(
                  'Background GDS did not report actual refusal: ${observed.readiness}/${observed.errorClass}',
                );
              }
              await tester.pump(const Duration(milliseconds: 200));
              await Future<void>.delayed(const Duration(milliseconds: 200));
            }
            final configured = (await device.settled(client.deviceModules))
                .singleWhere((module) => module.id == 'gds');
            if (Platform.isLinux || Platform.isMacOS) {
              expect(configured.optionalPermissions, contains('git_write'));
              expect(configured.permissions, isNot(contains('git_write')));
              await modules.configure(
                tester,
                enabled: true,
                permissions: [
                  'read_only',
                  'credential_store',
                  'local_network',
                  'git_write',
                ],
              );
              expect(
                (await device.settled(client.deviceModules))
                    .singleWhere((module) => module.id == 'gds')
                    .permissions,
                contains('git_write'),
              );
              await modules.configure(
                tester,
                enabled: true,
                permissions: ['read_only', 'credential_store', 'local_network'],
              );
              expect(
                (await device.settled(client.deviceModules))
                    .singleWhere((module) => module.id == 'gds')
                    .permissions,
                isNot(contains('git_write')),
              );
            } else {
              expect(
                configured.optionalPermissions,
                isNot(contains('git_write')),
              );
              expect(
                find.byKey(const Key('module-permission-git_write')),
                findsNothing,
              );
            }
            await modules.tapVisible(tester, find.text('NDS session'));
          }
        }
        await modules.tapVisible(tester, find.byKey(const Key('open-vault')));
        await waitFor(tester, find.text('Pending: 0 · Conflicts: 0'));
        // This explicit scenario owns source selection and convergence. General
        // identity/tool scenarios retain their separate consent requirements.
        return;
      }
      if (vault) {
        await modules.tapVisible(tester, find.text('Local modules'));
        await modules.waitFor(tester, find.byKey(const Key('module-selector')));
        await modules.choose(tester, 'Clipboard history');
        await modules.configure(
          tester,
          enabled: true,
          permissions: ['clipboard', 'local_network'],
        );
        await modules.tapVisible(tester, find.text('NDS session'));
        await tester.ensureVisible(find.byKey(const Key('open-utilities')));
        await tester.pumpAndSettle();
        await tester.tap(find.byKey(const Key('open-utilities')));
        await waitFor(tester, find.byKey(const Key('shared-text')));
        await tester.enterText(
          find.byKey(const Key('shared-text')),
          'Owned shared clipboard acceptance',
        );
        FocusManager.instance.primaryFocus?.unfocus();
        await tester.pumpAndSettle();
        await tester.ensureVisible(find.byKey(const Key('share-text')));
        await tester.tap(find.byKey(const Key('share-text')));
        await waitFor(tester, find.textContaining('Server revision'));
        if (!Platform.isIOS) {
          await tester.ensureVisible(
            find.byKey(const Key('copy-shared-clipboard')),
          );
          await tester.pumpAndSettle();
          await waitFor(
            tester,
            find.byWidgetPredicate(
              (widget) =>
                  widget is FilledButton &&
                  widget.key == const Key('copy-shared-clipboard') &&
                  widget.onPressed != null,
            ),
          );
          await tester.tap(find.byKey(const Key('copy-shared-clipboard')));
          await waitFor(tester, find.text('Copied to device clipboard.'));
          await tester.pumpAndSettle();
          final deadline = DateTime.now().add(const Duration(seconds: 10));
          while (true) {
            final copied = await Clipboard.getData(Clipboard.kTextPlain);
            if (copied?.text == 'Owned shared clipboard acceptance') break;
            if (DateTime.now().isAfter(deadline)) {
              final error = find.byKey(const Key('utility-error'));
              throw StateError(
                error.evaluate().isEmpty
                    ? 'Native clipboard value not observed'
                    : tester.widget<Text>(error).data!,
              );
            }
            await tester.pump(const Duration(milliseconds: 100));
            await Future<void>.delayed(const Duration(milliseconds: 100));
          }
          if (Platform.isAndroid) {
            await tester.ensureVisible(
              find.byKey(const Key('share-native-clipboard')),
            );
            await tester.pumpAndSettle();
            await tester.tap(find.byKey(const Key('share-native-clipboard')));
            await waitFor(
              tester,
              find.textContaining('This clipboard item is private'),
            );
          }
        }
        await tester.pageBack();
        await tester.pumpAndSettle();
      }
      if (phase == 'persist') {
        await tester.pumpWidget(const SizedBox.shrink());
        return;
      }

      // Replace the client. Restoration must use the actual native store and
      // a fresh server validation, rather than a retained Dart session object.
      await tester.ensureVisible(find.text('Change server'));
      await tester.tap(find.text('Change server'));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('connect')));
      await waitFor(tester, find.byKey(const Key('logout')));
      expect(
        find.descendant(
          of: currentCard(),
          matching: find.text('Acceptance device'),
        ),
        findsOneWidget,
      );
      expect(find.text('This installation'), findsOneWidget);
      final revoke = find.descendant(
        of: currentCard(),
        matching: find.widgetWithText(TextButton, 'Revoke device'),
      );
      await tester.ensureVisible(revoke);
      await tester.tap(revoke);
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(FilledButton, 'Revoke device'));
      await tester.pumpAndSettle();
      await waitFor(
        tester,
        find.descendant(
          of: currentCard(),
          matching: find.textContaining('· Revoked'),
        ),
      );
      await tester.ensureVisible(find.byKey(const Key('logout')));
      await tester.tap(find.byKey(const Key('logout')));
      await waitFor(tester, find.byKey(const Key('email')));
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );
}

bool nativeDesktop() =>
    Platform.isLinux || Platform.isMacOS || Platform.isWindows;

Future<void> writeScreenFact(
  String directory,
  String name,
  Map<String, Object> fact,
) async {
  final temporary = File('$directory/$name.tmp');
  await temporary.writeAsString(jsonEncode(fact), flush: true);
  await temporary.rename('$directory/$name');
}

Future<void> waitScreenFact(
  WidgetTester tester,
  String directory,
  String name,
) async {
  final deadline = DateTime.now().add(const Duration(seconds: 90));
  while (!await File('$directory/$name').exists()) {
    expect(
      DateTime.now().isBefore(deadline),
      isTrue,
      reason: 'Owned screen handoff timed out: $name',
    );
    await tester.pump(const Duration(milliseconds: 100));
    await Future<void>.delayed(const Duration(milliseconds: 100));
  }
}
