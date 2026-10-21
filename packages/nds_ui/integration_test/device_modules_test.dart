import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:nds_native/nds_native.dart' as native;
import 'package:nds_ui/module_settings.dart';
import 'package:nds_ui/nds_ui.dart';

import 'fixture_identity.dart' as identity;
import 'local_modules_test.dart' as controls;

const text = 'NDS Android module permission proof';
const capturedText = 'NDS Android native capture proof';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets('actual Android module consent and retained offline edits', (
    tester,
  ) async {
    expect(Platform.isAndroid, isTrue);
    const phase = String.fromEnvironment('NDS_TEST_PHASE');
    expect(['persist', 'offline', 'restore'], contains(phase));
    const server = String.fromEnvironment('NDS_TEST_SERVER');
    const mailbox = String.fromEnvironment('NDS_TEST_MAILPIT');
    const owner = String.fromEnvironment('NDS_TEST_OWNER');
    expect(Uri.parse(server).host, '127.0.0.1');
    expect(owner.endsWith('.invalid'), isTrue);
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
    await tester.enterText(find.byKey(const Key('server-address')), server);
    await controls.tapVisible(tester, find.byKey(const Key('connect')));
    if (phase == 'persist') {
      await identity.waitFor(tester, find.byKey(const Key('email')));
      await tester.enterText(find.byKey(const Key('email')), owner);
      final prior = await identity.localMessageIds(mailbox);
      await controls.tapVisible(tester, find.byKey(const Key('send-code')));
      await identity.waitFor(tester, find.byKey(const Key('otp')));
      await tester.enterText(
        find.byKey(const Key('otp')),
        await identity.readLocalCode(mailbox, owner, prior),
      );
      await controls.tapVisible(tester, find.byKey(const Key('verify')));
    }
    await identity.waitFor(tester, find.byKey(const Key('logout')));
    await identity.waitFor(tester, find.text('This installation'));
    await controls.tapVisible(tester, find.text('Local modules'));
    await controls.waitFor(tester, find.byKey(const Key('module-selector')));
    expect(native.supportsLocalAgent(), isFalse);
    expect(
      tester
          .widget<SwitchListTile>(find.byKey(const Key('module-enabled')))
          .onChanged,
      isNull,
    );
    await controls.choose(tester, 'Clipboard history');
    final client = tester
        .widget<ModuleSettings>(find.byType(ModuleSettings))
        .client;
    if (phase == 'persist') {
      await denied(client, 'module_disabled');
      await controls.configure(tester, enabled: true, permissions: []);
      await denied(client, 'module_permission_required');
      await controls.configure(
        tester,
        enabled: true,
        permissions: ['clipboard', 'local_network'],
      );
      await settled(() => client.clipboardPublishText(text: text));
    } else if (phase == 'offline') {
      final before = (await settled(client.deviceModules))
          .singleWhere((module) => module.id == 'clipboard');
      expect(before.enabled, isTrue);
      expect(
        before.permissions,
        unorderedEquals(['clipboard', 'local_network']),
      );
      expect(client.snapshot().offlineAuthorized, isTrue);
      await controls.configure(
        tester,
        enabled: true,
        permissions: ['local_network'],
      );
      await denied(client, 'module_permission_required');
      final after = (await settled(client.deviceModules))
          .singleWhere((module) => module.id == 'clipboard');
      expect(after.pendingId, isNotNull);
      expect((await settled(client.vaultSnapshot)).pending, greaterThan(0));
      expect((await settled(client.vaultSnapshot)).conflicts, 0);
    } else {
      final retained = (await settled(client.deviceModules))
          .singleWhere((module) => module.id == 'clipboard');
      expect(retained.enabled, isTrue);
      expect(retained.permissions, ['local_network']);
      await denied(client, 'module_permission_required');
      await drained(client);
      await controls.configure(
        tester,
        enabled: true,
        permissions: ['clipboard', 'local_network'],
      );
    }
    if (phase != 'offline') {
      await drained(client);
      final value = await settled(client.clipboardSnapshot);
      expect(value.present, isTrue);
      expect(value.serverSeq, greaterThan(BigInt.zero));
      final expectedText = phase == 'persist' ? text : capturedText;
      expect(value.bytes, expectedText.length);
      expect(native.supportsNativeClipboardRead(), isTrue);
      await settled(() => client.clipboardCopy());
      // These platform-channel calls use the actual Android clipboard. Only a
      // Boolean comparison is asserted, so unexpected contents never enter logs.
      expect(
        (await Clipboard.getData('text/plain'))?.text == expectedText,
        isTrue,
      );
      await denied(
        client,
        'clipboard_private',
        operation: client.clipboardPublishNative,
      );
      await Clipboard.setData(const ClipboardData(text: capturedText));
      await settled(client.clipboardPublishNative);
      await drained(client);
      expect(
        (await settled(client.clipboardSnapshot)).bytes,
        capturedText.length,
      );
      final observed = (await settled(client.deviceModules))
          .singleWhere((module) => module.id == 'clipboard');
      expect(observed.readiness, 'ready');
      expect(observed.checkedAtMs, isNotNull);
      await settled(client.publishDeviceState);
      await drained(client);
      final states = await settled(() => client.syncStates(after: ''));
      final published = states.items.singleWhere(
        (value) =>
            value.kind == 'module_state' &&
            value.id == '${client.snapshot().currentDeviceId}:clipboard',
      );
      expect(published.configured, isTrue);
      expect(published.reachability, 'reachable');
      expect(published.checkedAtMs, observed.checkedAtMs);
      expect(published.revision, greaterThan(BigInt.zero));
      expect(published.pendingId, isNull);
      if (phase == 'restore') {
        await controls.configure(tester, enabled: false, permissions: []);
        await denied(client, 'module_disabled');
        await drained(client);
      }
    }
    final diagnostics = native.clientDiagnostics();
    expect(diagnostics.sinkInitialized, isTrue);
    expect(diagnostics.writeFailures, BigInt.zero);
    expect(diagnostics.droppedEvents, BigInt.zero);
    expect(diagnostics.rejectedFieldsOrEvents, BigInt.zero);
    if (phase != 'offline' &&
        client.deliverySnapshot().policy == native.ExportPolicy.enabled) {
      // The supervisor deliberately replaces this process between phases.
      // Wait for the actual sender before discarding its native RAM queues.
      final deadline = DateTime.now().add(const Duration(seconds: 30));
      while (true) {
        final delivery = client.deliverySnapshot();
        expect(delivery.dropped, BigInt.zero);
        expect(delivery.spansDropped, BigInt.zero);
        if (delivery.pending == 0 && delivery.spansPending == 0) {
          expect(delivery.delivered, greaterThan(BigInt.zero));
          expect(delivery.spansDelivered, greaterThan(BigInt.zero));
          break;
        }
        if (DateTime.now().isAfter(deadline)) {
          throw StateError('Module telemetry was not acknowledged before exit');
        }
        await Future<void>.delayed(const Duration(milliseconds: 200));
      }
    }
    expect(tester.takeException(), isNull);
    debugPrint('NDS_ANDROID_MODULE_PHASE_PASS phase=$phase');
  }, timeout: const Timeout(Duration(minutes: 3)));
}

// Expected bounded contention is retried; authority or permission failures are
// retained. No fixture account, native adapter or production state is replaced.
Future<T> settled<T>(Future<T> Function() work) async {
  final deadline = DateTime.now().add(const Duration(seconds: 30));
  while (true) {
    try {
      return await work();
    } catch (error) {
      final retry =
          error == native.VaultError.busy ||
          error is native.ClientError &&
              {
                'busy',
                'vault_busy',
                'online_verification_required',
              }.contains(error.code());
      if (!retry) rethrow;
      if (error is native.ClientError) error.dispose();
      if (DateTime.now().isAfter(deadline)) {
        throw StateError('Actual module operation remained busy');
      }
      await Future<void>.delayed(const Duration(milliseconds: 200));
    }
  }
}

Future<void> denied(
  native.NdsClient client,
  String expected, {
  Future<String> Function()? operation,
}) async {
  try {
    await settled(operation ?? () => client.clipboardPublishText(text: text));
    fail('The module action should have been refused');
  } on native.ClientError catch (error) {
    expect(error.code(), expected);
    error.dispose();
  }
}

Future<void> drained(native.NdsClient client) async {
  final deadline = DateTime.now().add(const Duration(seconds: 30));
  while (true) {
    final view = await settled(client.vaultSnapshot);
    expect(view.conflicts, 0);
    if (view.pending == 0 && !view.catchingUp) return;
    if (DateTime.now().isAfter(deadline)) {
      throw StateError(
        'Retained module edits did not synchronize automatically',
      );
    }
    await Future<void>.delayed(const Duration(milliseconds: 200));
  }
}
