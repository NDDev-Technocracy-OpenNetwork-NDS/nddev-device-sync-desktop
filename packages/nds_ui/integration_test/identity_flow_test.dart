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
import 'package:nds_ui/vault_screen.dart';
import 'package:nds_ui/l10n/app_localizations.dart';
import 'package:nds_native/nds_native.dart' as native;

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
  const appUpdateHandoff = String.fromEnvironment(
    'NDS_TEST_APP_UPDATE_HANDOFF',
  );
  const screenHandoff = String.fromEnvironment('NDS_TEST_SCREEN_HANDOFF');
  const screenCase = String.fromEnvironment(
    'NDS_TEST_SCREEN_CASE',
    defaultValue: 'role',
  );
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
      if (appUpdateHandoff.isNotEmpty) {
        expect(vault && Platform.isLinux && screenPeer.isEmpty, isTrue);
        await appUpdatePreservation(tester, server, appUpdateHandoff);
        expect(tester.takeException(), isNull);
        return;
      }
      if (screenPeer.isNotEmpty) {
        expect(Platform.isLinux, isTrue);
        expect([
          'role',
          'source-loss',
          'cancel',
          'local-disable',
          'logout',
          'expiry',
        ], contains(screenCase));
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
        await waitScreenFact(tester, screenHandoff, 'end-trigger.json');
        final stopped = Stopwatch()..start();
        while (image().evaluate().isNotEmpty) {
          expect(
            stopped.elapsed <
                Duration(seconds: screenCase == 'expiry' ? 330 : 10),
            isTrue,
            reason: 'Flutter retained a screen image after $screenCase',
          );
          await tester.pump(const Duration(milliseconds: 100));
          await Future<void>.delayed(const Duration(milliseconds: 100));
        }
        final observedStopMs = stopped.elapsedMilliseconds;
        if (screenCase == 'expiry') {
          await waitScreenFact(tester, screenHandoff, 'host-proof.json');
          final hostProof = jsonDecode(
            await File('$screenHandoff/host-proof.json').readAsString(),
          ) as Map<String, dynamic>;
          expect(
            observedStopMs + 1000,
            greaterThanOrEqualTo(hostProof['observed_stop_ms'] as int),
            reason: 'Flutter stopped before the actual native expiry boundary',
          );
        }
        await writeScreenFact(screenHandoff, 'viewer-proof.json', {
          'stopped': true,
          'image_removed': true,
          'observed_stop_ms': observedStopMs,
        });
        await tester.pageBack();
        if (screenCase == 'role') {
          await waitFor(tester, find.byKey(const Key('screen-targets-error')));
          expect(find.text('Owned screen host'), findsNothing);
        } else {
          await waitFor(tester, find.byKey(const Key('screen-targets')));
          expect(find.byKey(const Key('screen-targets-error')), findsNothing);
        }
        await tester.pageBack();
        if (screenCase == 'role') {
          await waitFor(tester, find.text('User'));
          expect(find.byKey(const Key('company-email')), findsNothing);
        } else {
          await waitFor(tester, find.byKey(const Key('company-email')));
        }
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

// Opt-in application staging preservation. Every account, key and operation is
// produced by the ordinary UI/Rust path against the existing owned fixture.
Future<void> appUpdatePreservation(
  WidgetTester tester,
  String server,
  String handoff,
) async {
  final directory = Directory(handoff);
  expect(directory.isAbsolute && directory.existsSync(), isTrue);
  expect(
    directory.uri.pathSegments
        .where((s) => s.isNotEmpty)
        .last
        .startsWith('nds-app-update-ui-'),
    isTrue,
  );
  final expectation = jsonDecode(
    await File('$handoff/expectation.json').readAsString(),
  ) as Map<String, dynamic>;
  final version = expectation['version'] as String;
  Future<void> until(Future<bool> Function() ready) async {
    final deadline = DateTime.now().add(const Duration(seconds: 90));
    while (!await ready()) {
      if (DateTime.now().isAfter(deadline)) {
        throw StateError('Owned application update phase timed out');
      }
      await tester.pump(const Duration(milliseconds: 100));
      await Future<void>.delayed(const Duration(milliseconds: 100));
    }
    await tester.pump();
  }

  Future<void> step(
    String name, [
    Map<String, Object?> values = const {},
  ]) async {
    expect(RegExp(r'^[a-z-]+$').hasMatch(name), isTrue);
    final fact = File('$handoff/$name.json');
    expect(await fact.exists(), isFalse);
    final temp = File('${fact.path}.tmp');
    await temp.writeAsString(jsonEncode(values), flush: true);
    await temp.rename(fact.path);
    final ack = File('$handoff/$name.ready');
    await until(() async => ack.exists());
    final result = jsonDecode(await ack.readAsString()) as Map<String, dynamic>;
    expect(result['ok'], isTrue, reason: 'Owned controller rejected $name');
  }

  Future<void> tap(String key) async {
    FocusManager.instance.primaryFocus?.unfocus();
    final finder = find.byKey(Key(key));
    await until(() async => finder.evaluate().isNotEmpty);
    await until(() async {
      final widget = tester.widget<Widget>(finder);
      if (widget is ButtonStyleButton) return widget.onPressed != null;
      if (widget is IconButton) return widget.onPressed != null;
      return true;
    });
    await tester.ensureVisible(finder);
    await tester.pump();
    await tester.tap(finder);
    await tester.pump();
  }

  Future<void> message(String expected) => until(() async {
    final found = find.byKey(const Key('app-update-status'));
    return found.evaluate().isNotEmpty &&
        tester.widget<Text>(found).data == expected;
  });
  Future<void> checkEnabled() => until(() async {
    final found = find.byKey(const Key('app-update-check'));
    return found.evaluate().isNotEmpty &&
        tester.widget<FilledButton>(found).onPressed != null;
  });
  Future<void> writeRecord(String title, String value) async {
    final editor = find.byKey(const Key('vault-title'));
    await until(
      () async =>
          editor.evaluate().isNotEmpty &&
          tester.widget<TextField>(editor).enabled == true,
    );
    await tester.enterText(editor, title);
    await tester.enterText(find.byKey(const Key('vault-secret')), value);
    await tap('vault-save');
    // Entered text is already findable before save completes. Require the
    // actual rendered record card and the cleared editor, not that draft.
    await until(
      () async =>
          find
              .descendant(of: find.byType(Card), matching: find.text(title))
              .evaluate()
              .isNotEmpty &&
          tester
              .widget<TextField>(find.byKey(const Key('vault-secret')))
              .controller!
              .text
              .isEmpty,
    );
    expect(find.text(value), findsNothing);
  }

  await tap('open-vault');
  await until(() async => find.byType(VaultScreen).evaluate().isNotEmpty);
  final original = tester.widget<VaultScreen>(find.byType(VaultScreen)).client;
  await writeRecord(
    'App update acknowledged baseline',
    'SYNTHETIC_APP_UPDATE_BASELINE',
  );
  await until(() async {
    try {
      final v = await original.vaultSnapshot();
      return v.unlocked && v.pending == 0 && !v.catchingUp;
    } on native.VaultError catch (e) {
      if (e.code() == 'vault_busy') return false;
      rethrow;
    }
  });
  final initial = await original.vaultSnapshot();
  final device = original.snapshot().currentDeviceId;
  expect(
    initial.unlocked && initial.keyId.isNotEmpty && device.isNotEmpty,
    isTrue,
  );
  await step('online-ready', {'device_id': device, 'key_id': initial.keyId});
  await step('offline-requested');
  await writeRecord(
    'App update pending original',
    'SYNTHETIC_APP_UPDATE_PENDING',
  );
  native.RecordView? retained;
  await until(() async {
    try {
      final page = await original.vaultRecords(after: '');
      for (final row in page.items) {
        if (row.title == 'App update pending original' &&
            row.pendingId != null) {
          retained = row;
          return true;
        }
      }
      return false;
    } on native.VaultError catch (e) {
      if (e.code() == 'vault_busy') return false;
      rethrow;
    }
  });
  final record = retained!;
  await step('pending-created', {
    'record_id': record.id,
    'pending_id': record.pendingId,
    'device_id': device,
    'key_id': initial.keyId,
  });
  await tester.pageBack();
  await tester.pumpAndSettle();
  await step('discovery-failure');
  await tap('app-update-open');
  await checkEnabled();
  final l = AppLocalizations.of(
    tester.element(find.byKey(const Key('app-update-status'))),
  )!;
  await tap('app-update-check');
  await message(l.appUpdateNetwork);
  await step('after-discovery');
  await step('partial-cancel');
  await tap('app-update-check');
  await message(l.appUpdateAvailable);
  expect(find.text(l.appUpdateVersion(version)), findsOneWidget);
  await tap('app-update-download');
  await until(
    () async => find.byType(Text).evaluate().any((e) {
      final text = (e.widget as Text).data ?? '';
      final match = RegExp(r'^([0-9]+) / ([0-9]+) ').firstMatch(text);
      return match != null &&
          BigInt.parse(match[1]!) > BigInt.zero &&
          BigInt.parse(match[1]!) < BigInt.parse(match[2]!);
    }),
  );
  await step('download-partial');
  await tap('app-update-cancel');
  await message(l.appUpdateCancelled);
  await step('after-cancel');
  await step('verified-stage');
  await tap('app-update-check');
  await message(l.appUpdateAvailable);
  await tap('app-update-download');
  await message(l.appUpdateStaged);
  await step('after-stage');
  await tester.pageBack();
  await tester.pumpAndSettle();
  await step('local-restore');
  await tap('app-update-open');
  await message(l.appUpdateStaged);
  await tester.pageBack();
  await tester.pumpAndSettle();
  // A fresh facade has no retained VaultClient/runtime slot: refresh_local must
  // read the actual protected native key and decrypt this same journal. Never
  // export keys/session values or attach a competing replica owner here.
  final fresh = native.NdsClient(endpoint: server);
  try {
    final view = await fresh.refresh();
    expect(view.authenticated && view.offlineAuthorized, isTrue);
    expect(view.currentDeviceId, device);
    final restored = await fresh.vaultSnapshot();
    expect(restored.unlocked, isTrue);
    expect(restored.keyId, initial.keyId);
    final page = await fresh.vaultRecords(after: '');
    expect(
      page.items.any(
        (r) =>
            r.id == record.id &&
            r.pendingId == record.pendingId &&
            r.title == record.title,
      ),
      isTrue,
    );
  } finally {
    fresh.cancel();
    fresh.dispose();
  }
  await tap('open-vault');
  expect(
    identical(
      tester.widget<VaultScreen>(find.byType(VaultScreen)).client,
      original,
    ),
    isTrue,
  );
  await tester.pageBack();
  await tester.pumpAndSettle();
  await step('after-restore', {
    'device_id': device,
    'key_id': initial.keyId,
    'fresh_native_store_and_vault_read': true,
  });
  await step('reconnect-requested');
  // No vaultSync/manual retry, replacement facade, logout, or owner stop. The
  // original background owner performs the upload after server recovery.
  await until(() async {
    try {
      final page = await original.vaultRecords(after: '');
      return page.items.any(
        (r) =>
            r.id == record.id &&
            r.pendingId == null &&
            r.observedSeq > BigInt.zero,
      );
    } on native.VaultError catch (e) {
      if (e.code() == 'vault_busy') return false;
      rethrow;
    }
  });
  final finalView = await original.vaultSnapshot();
  expect(
    finalView.unlocked && finalView.pending == 0 && finalView.conflicts == 0,
    isTrue,
  );
  expect(finalView.keyId, initial.keyId);
  expect(original.snapshot().currentDeviceId, device);
  await step('after-reconnect', {
    'device_id': device,
    'key_id': finalView.keyId,
    'manual_sync': false,
  });
  // Ordinary logout of this synthetic fixture account is cleanup, not update
  // activation or a stop of an ambient service.
  await tap('logout');
  await until(() async => find.byKey(const Key('email')).evaluate().isNotEmpty);
  await step('complete', {'passed': true, 'fixture_signed_out': true});
  debugPrint('NDS_APP_UPDATE_PRESERVATION_PASS');
}
