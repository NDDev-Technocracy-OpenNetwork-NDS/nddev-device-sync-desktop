// Real owned loopback fixture helpers; no identity/provider substitution.
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

Future<void> waitFor(WidgetTester tester, Finder finder) async {
  final deadline = DateTime.now().add(const Duration(seconds: 25));
  while (finder.evaluate().isEmpty) {
    if (DateTime.now().isAfter(deadline)) {
      final error = find.byWidgetPredicate(
        (widget) =>
            widget is Text &&
            {
              const Key('auth-error'),
              const Key('vault-error'),
              const Key('state-error'),
              const Key('utility-error'),
              const Key('module-error'),
              const Key('company-error'),
            }.contains(widget.key),
      );
      final message = error.evaluate().isEmpty
          ? 'no displayed error'
          : tester.widget<Text>(error).data;
      throw StateError(
        'Identity UI did not reach its expected state: $message',
      );
    }
    await tester.pump(const Duration(milliseconds: 100));
    await Future<void>.delayed(const Duration(milliseconds: 100));
  }
  await tester.pumpAndSettle();
}

Future<String> readLocalCode(
  String mailbox,
  String recipient,
  Set<String> prior,
) async {
  final client = HttpClient()..connectionTimeout = const Duration(seconds: 5);
  Future<Map<String, dynamic>> get(String path) async {
    final request = await client.getUrl(Uri.parse('$mailbox$path'));
    final response = await request.close().timeout(const Duration(seconds: 5));
    if (response.statusCode != 200) {
      throw StateError('Isolated mailbox unavailable');
    }
    final body = await utf8.decoder.bind(response).join();
    return jsonDecode(body) as Map<String, dynamic>;
  }

  try {
    return await (() async {
      while (true) {
        final messages = (await get('/api/v1/messages'))['messages'] as List;
        for (final row in messages) {
          final id = (row as Map)['ID'] as String;
          if (prior.contains(id)) continue;
          final message = await get('/api/v1/message/$id');
          if (!(message['To'] as List).any(
            (to) => (to as Map)['Address'] == recipient,
          )) {
            continue;
          }
          final match = RegExp(r'(?:code|код входа): (\d{8})')
              .firstMatch(message['Text'] as String);
          if (match != null) return match.group(1)!;
        }
        await Future<void>.delayed(const Duration(milliseconds: 100));
      }
    })().timeout(const Duration(seconds: 10));
  } finally {
    client.close(force: true);
  }
}

Future<Set<String>> localMessageIds(String mailbox) async {
  final client = HttpClient()..connectionTimeout = const Duration(seconds: 5);
  try {
    final request = await client.getUrl(Uri.parse('$mailbox/api/v1/messages'));
    final response = await request.close().timeout(const Duration(seconds: 5));
    if (response.statusCode != 200) {
      throw StateError('Owned mailbox unavailable');
    }
    final body = jsonDecode(await utf8.decoder.bind(response).join()) as Map;
    return (body['messages'] as List)
        .map((row) => (row as Map)['ID'] as String)
        .toSet();
  } finally {
    client.close(force: true);
  }
}
