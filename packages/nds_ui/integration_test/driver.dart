import 'dart:io';

import 'package:integration_test/integration_test_driver.dart';

Future<void> main() => integrationDriver(
  // The existing expiry case retains its five-minute grant budget. The
  // separate app-update preservation scenario owns its bounded named budget.
  timeout: Duration(
    minutes:
        Platform.environment['NDS_TEST_APP_UPDATE_CASE'] == 'preservation' ||
            Platform.environment['NDS_TEST_SCREEN_CASE'] == 'expiry'
        ? 8
        : 4,
  ),
);
