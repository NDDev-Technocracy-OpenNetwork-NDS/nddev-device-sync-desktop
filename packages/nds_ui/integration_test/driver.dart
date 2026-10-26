import 'dart:io';

import 'package:integration_test/integration_test_driver.dart';

Future<void> main() => integrationDriver(
  // Only the owned expiry case waits for a real five-minute native grant.
  timeout: Duration(
    minutes: Platform.environment['NDS_TEST_SCREEN_CASE'] == 'expiry' ? 8 : 4,
  ),
);
