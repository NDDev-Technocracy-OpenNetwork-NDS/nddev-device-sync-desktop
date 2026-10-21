import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:nds_ui/nds_ui.dart';

// Devices run sequentially against the same server. The first process is gone
// before the new device obtains its key; no peer relay or key transfer is used.
void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  const server = String.fromEnvironment('NDS_TEST_SERVER');
  const mailbox = String.fromEnvironment('NDS_TEST_MAILPIT');
  const owner = String.fromEnvironment('NDS_TEST_OWNER');
  const role = String.fromEnvironment('NDS_TEST_SYNC_ROLE');
  const phase = String.fromEnvironment('NDS_TEST_PHASE');

  testWidgets(
    'actual server-managed provisioning with the other device offline',
    (tester) async {
      for (final url in [server, mailbox]) {
        expect(Uri.parse(url).host, '127.0.0.1');
      }
      expect(owner.endsWith('.invalid'), isTrue);
      final http = HttpClient()..connectionTimeout = const Duration(seconds: 3);
      http.findProxy = (_) => 'DIRECT';
      final framework = FlutterError.onError;
      await runNdsApp();
      final diagnostic = FlutterError.onError;
      FlutterError.onError = (details) {
        diagnostic?.call(details);
        framework?.call(details);
      };
      addTearDown(() async {
        tester.binding.handleAppLifecycleStateChanged(
          AppLifecycleState.resumed,
        );
        await tester.pumpWidget(const SizedBox.shrink());
        FlutterError.onError = framework;
        http.close(force: true);
      });
      Future<Map<String, dynamic>> request(
        String url, [
        Map<String, dynamic>? body,
      ]) async {
        final req = await http.openUrl(
          body == null ? 'GET' : 'POST',
          Uri.parse(url),
        );
        if (body != null) {
          req.headers.contentType = ContentType.json;
          final bytes = utf8.encode(jsonEncode(body));
          req.contentLength = bytes.length;
          req.add(bytes);
        }
        final res = await req.close().timeout(const Duration(seconds: 5));
        expect(res.statusCode, 200);
        return jsonDecode(await utf8.decoder.bind(res).join())
            as Map<String, dynamic>;
      }

      Future<void> waitUntil(bool Function() ready) async {
        final deadline = DateTime.now().add(const Duration(seconds: 160));
        while (!ready()) {
          if (DateTime.now().isAfter(deadline)) {
            throw StateError('Replica acceptance timed out');
          }
          await tester.pump(const Duration(milliseconds: 100));
          await Future<void>.delayed(const Duration(milliseconds: 100));
        }
        // A confirmation dialog intentionally keeps the operation busy. Waiting
        // for every animation behind it to settle would prevent confirmation.
        await tester.pump(const Duration(milliseconds: 300));
      }

      Future<void> visible(String key) async {
        FocusManager.instance.primaryFocus?.unfocus();
        await tester.pumpAndSettle();
        final finder = find.byKey(Key(key));
        await tester.ensureVisible(finder);
        await tester.pumpAndSettle();
        await tester.tap(finder);
      }

      await tester.pumpAndSettle();
      await tester.enterText(find.byKey(const Key('server-address')), server);
      await visible('connect');
      if (phase != 'restore') {
        await waitUntil(
          () => find.byKey(const Key('email')).evaluate().isNotEmpty,
        );
        await tester.enterText(find.byKey(const Key('email')), owner);
        final requestedAt = DateTime.now();
        await visible('send-code');
        await waitUntil(
          () => find.byKey(const Key('otp')).evaluate().isNotEmpty,
        );
        final messages =
            (await request('$mailbox/api/v1/messages'))['messages'] as List;
        final id = (messages.first as Map)['ID'] as String;
        final message = await request('$mailbox/api/v1/message/$id');
        final code = RegExp(r'code: (\d{8})')
            .firstMatch(message['Text'] as String)!
            .group(1)!;
        await tester.enterText(find.byKey(const Key('otp')), code);
        await visible('verify');
        await waitUntil(
          () => find.text('This installation').evaluate().isNotEmpty,
        );
        await visible('open-vault');
        await waitUntil(
          () => find.byKey(const Key('vault-title')).evaluate().isNotEmpty,
        );
        if (role == 'owner') {
          // A visible desktop window may lose input focus while the owner edits.
          // Local save must still converge through the real background replica.
          tester.binding.handleAppLifecycleStateChanged(
            AppLifecycleState.inactive,
          );
          await tester.enterText(
            find.byKey(const Key('vault-title')),
            'Linux shared record',
          );
          await tester.enterText(
            find.byKey(const Key('vault-secret')),
            'PRIVATE_PAIR_LINUX_VALUE',
          );
          await visible('vault-save');
          await waitUntil(
            () => find.text('Linux shared record').evaluate().isNotEmpty,
          );
          await waitUntil(
            () => find.text('Pending: 0 · Conflicts: 0').evaluate().isNotEmpty,
          );
          tester.binding.handleAppLifecycleStateChanged(
            AppLifecycleState.resumed,
          );
          await tester.pageBack();
          await tester.pumpAndSettle();
          await tester.tap(find.text('Local modules'));
          await waitUntil(
            () => find
                .byKey(const Key('publish-local-state'))
                .evaluate()
                .isNotEmpty,
          );
          await visible('publish-local-state');
          await waitUntil(
            () => find
                .text('Local states queued for synchronization.')
                .evaluate()
                .isNotEmpty,
          );
          await tester.tap(find.text('NDS session'));
          await tester.pumpAndSettle();
          await visible('open-vault');
          await waitUntil(
            () => find.text('Pending: 0 · Conflicts: 0').evaluate().isNotEmpty,
          );
          // Keep the actual OTP resend policy; the next independent process can
          // sign in only after the real cooldown, without changing clocks.
          await waitUntil(
            () => DateTime.now().difference(requestedAt).inSeconds >= 61,
          );
        } else {
          await waitUntil(
            () => find.text('Linux shared record').evaluate().isNotEmpty,
          );
          await tester.enterText(
            find.byKey(const Key('vault-title')),
            'Android shared record',
          );
          await tester.enterText(
            find.byKey(const Key('vault-secret')),
            'PRIVATE_PAIR_ANDROID_VALUE',
          );
          await visible('vault-save');
          await waitUntil(
            () => find.text('Android shared record').evaluate().isNotEmpty,
          );
          await visible('open-synced-state');
          await waitUntil(
            () => find
                .byKey(const Key('publish-device-state'))
                .evaluate()
                .isNotEmpty,
          );
          await visible('publish-device-state');
          await waitUntil(() => find.text('gds').evaluate().isNotEmpty);
          expect(find.text('rds'), findsOneWidget);
          expect(find.text('Sync owner'), findsOneWidget);
          await tester.pageBack();
          await tester.pumpAndSettle();
        }
      } else {
        await waitUntil(
          () => find.byKey(const Key('open-vault')).evaluate().isNotEmpty,
        );
        await visible('open-vault');
        await waitUntil(
          () => find.text('Linux shared record').evaluate().isNotEmpty,
        );
        expect(find.text('Android shared record'), findsOneWidget);
        expect(find.byKey(const Key('vault-create')), findsNothing);
        await visible('open-synced-state');
        await waitUntil(() => find.text('gds').evaluate().isNotEmpty);
        expect(find.text('Sync owner'), findsOneWidget);
      }
      expect(find.text('PRIVATE_PAIR_LINUX_VALUE'), findsNothing);
      expect(find.text('PRIVATE_PAIR_ANDROID_VALUE'), findsNothing);
      expect(tester.takeException(), isNull);
    },
  );
}
