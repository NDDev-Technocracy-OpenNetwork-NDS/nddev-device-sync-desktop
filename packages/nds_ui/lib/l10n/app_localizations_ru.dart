// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Russian (`ru`).
class AppLocalizationsRu extends AppLocalizations {
  AppLocalizationsRu([String locale = 'ru']) : super(locale);

  @override
  String get appTitle => 'NDDev OpenNetwork';

  @override
  String get connectTitle => 'Подключение к NDS';

  @override
  String get connectHelp => 'Укажите HTTPS-адрес вашего сервера NDS.';

  @override
  String get serverAddress => 'Адрес сервера';

  @override
  String get connect => 'Подключиться';

  @override
  String get signIn => 'Вход';

  @override
  String get email => 'Электронная почта';

  @override
  String get sendCode => 'Получить код входа';

  @override
  String get emailHint => 'Если адрес разрешён, на него придёт код входа.';

  @override
  String get code => 'Восьмизначный код';

  @override
  String get verify => 'Подтвердить код';

  @override
  String get resend => 'Отправить новый код';

  @override
  String get github => 'Войти через GitHub';

  @override
  String get githubHelp =>
      'Сравните этот код с кодом на странице подтверждения NDS в браузере. Разрешайте вход, только если коды совпадают.';

  @override
  String get openGithub => 'Открыть GitHub';

  @override
  String get awaitApproval => 'Ожидается подтверждение в браузере';

  @override
  String get cancel => 'Отменить вход';

  @override
  String get unavailable => 'Недоступно на этом сервере';

  @override
  String get sessionTitle => 'Ваша сессия NDS';

  @override
  String get user => 'Пользователь';

  @override
  String get instance => 'Сервер';

  @override
  String get method => 'Способ входа';

  @override
  String get expires => 'Действует до';

  @override
  String get verified => 'Последняя проверка';

  @override
  String get unverified =>
      'Сессия не подтверждена повторно. Проверьте подключение к серверу.';

  @override
  String get refresh => 'Проверить сессию';

  @override
  String get logout => 'Отозвать сессию и выйти';

  @override
  String get changeServer => 'Сменить сервер';

  @override
  String get diagnostics => 'Диагностические логи на пять минут';

  @override
  String get diagnosticsOff => 'Отключить диагностические логи';

  @override
  String get privacy =>
      'Данные сессии и ключи vault хранятся в защищённом хранилище ОС. Сервер передаёт ключ vault после подтверждённого входа и регистрации устройства.';

  @override
  String get endpointError =>
      'Укажите HTTPS-адрес сервера без пароля, пути, параметров и фрагмента.';

  @override
  String get inputError => 'Проверьте адрес почты и восьмизначный код.';

  @override
  String get networkError =>
      'Сервер недоступен. Актуальное состояние сессии не подтверждено.';

  @override
  String get storeError =>
      'Защищённое хранилище учётных данных недоступно. Сохранение открытым текстом не используется.';

  @override
  String get authError =>
      'Не удалось подтвердить вход. Проверьте код или начните заново.';

  @override
  String get expiredError => 'Время для входа истекло. Начните заново.';

  @override
  String get rateError => 'Подождите перед следующей попыткой.';

  @override
  String get cancelledError => 'Вход отменён.';

  @override
  String get responseError => 'Ответ сервера не соответствует протоколу NDS.';

  @override
  String get genericError =>
      'Не удалось выполнить действие. Повторите попытку.';

  @override
  String get launchError => 'Не удалось открыть браузер для входа.';

  @override
  String get working => 'Выполняется…';

  @override
  String validFor(int seconds) {
    String _temp0 = intl.Intl.pluralLogic(
      seconds,
      locale: localeName,
      other: 'Действует ещё $seconds секунды',
      many: 'Действует ещё $seconds секунд',
      few: 'Действует ещё $seconds секунды',
      one: 'Действует ещё $seconds секунду',
    );
    return '$_temp0';
  }

  @override
  String retryIn(int seconds) {
    String _temp0 = intl.Intl.pluralLogic(
      seconds,
      locale: localeName,
      other: 'Повтор через $seconds секунды',
      many: 'Повтор через $seconds секунд',
      few: 'Повтор через $seconds секунды',
      one: 'Повтор через $seconds секунду',
    );
    return '$_temp0';
  }

  @override
  String get revalidationRequired =>
      'Секрет сессии мог сохраниться. Проверьте сессию перед продолжением.';

  @override
  String get emailMethod => 'Код по почте';

  @override
  String get sessionTab => 'Сессия NDS';

  @override
  String get localModules => 'Локальные модули';

  @override
  String get localReadOnly =>
      'Просмотр состояния инструментов, истории и плана обслуживания. Данные остаются у своих инструментов.';

  @override
  String get module => 'Модуль';

  @override
  String get systemInfo => 'Состояние системы';

  @override
  String get clipboardHistory => 'История буфера обмена';

  @override
  String get cleaner => 'Очистка';

  @override
  String get updater => 'Обновления';

  @override
  String get providerUnavailable => 'Локальный инструмент недоступен.';

  @override
  String get providerFailed => 'Локальный инструмент не выполнил запрос.';

  @override
  String get notChecked => 'Ещё не проверено';

  @override
  String get reachable => 'Доступен';

  @override
  String get noRecords => 'Записей нет.';

  @override
  String get nextPage => 'Следующая страница';

  @override
  String get openText => 'Прочитать текст';

  @override
  String get cpuUsage => 'Загрузка CPU';

  @override
  String get cpuTemperature => 'Температура CPU';

  @override
  String get memoryUsage => 'Использование памяти';

  @override
  String get swapUsage => 'Использование swap';

  @override
  String get gpuUsage => 'Загрузка GPU';

  @override
  String get gpuMemory => 'Использование памяти GPU';

  @override
  String get gpuTemperature => 'Температура GPU';

  @override
  String get diskRead => 'Чтение диска';

  @override
  String get diskWrite => 'Запись диска';

  @override
  String get diskTemperature => 'Температура диска';

  @override
  String get networkReceive => 'Приём данных';

  @override
  String get networkSend => 'Передача данных';

  @override
  String get errors => 'Ошибки';

  @override
  String get warnings => 'Предупреждения';

  @override
  String get checkOk => 'В порядке';

  @override
  String get checkWarning => 'Предупреждение';

  @override
  String get checkError => 'Ошибка';

  @override
  String get checkInfo => 'Сведения';

  @override
  String get mode => 'Режим';

  @override
  String get preview => 'Предпросмотр';

  @override
  String get executed => 'Выполнено';

  @override
  String get duration => 'Длительность';

  @override
  String get freedBytes => 'Освобождено';

  @override
  String get deleted => 'Удалено записей';

  @override
  String get skipped => 'Пропущено записей';

  @override
  String get failed => 'Не обработано записей';

  @override
  String get channel => 'Канал';

  @override
  String get result => 'Результат';

  @override
  String get exitClass => 'Класс завершения';

  @override
  String get branch => 'Ветка';

  @override
  String get upstream => 'Отслеживаемая ветка';

  @override
  String get aheadBehind => 'Впереди / позади';

  @override
  String get gdsUnavailable => 'Недоступно';

  @override
  String get gdsComparisonNotApplicable => 'Неприменимо';

  @override
  String get staged => 'Подготовлено изменений';

  @override
  String get unstaged => 'Изменений вне индекса';

  @override
  String get untracked => 'Неотслеживаемых файлов';

  @override
  String get conflicts => 'Конфликты';

  @override
  String get remoteFreshness => 'Актуальность удалённых данных';

  @override
  String get repositoryPath => 'Абсолютный путь репозитория';

  @override
  String get lastRun => 'Последний запуск';

  @override
  String get updatePlan => 'План обновления';

  @override
  String get moduleRefresh => 'Обновить';

  @override
  String get moduleCancel => 'Отменить запрос';

  @override
  String get moduleDiagnostics => 'Диагностика';

  @override
  String get moduleInputError =>
      'Проверьте путь репозитория или выбранную запись.';

  @override
  String get moduleNetworkError => 'Локальный инструмент не ответил вовремя.';

  @override
  String get moduleResponseError =>
      'Локальный инструмент вернул неподдерживаемый ответ.';

  @override
  String get moduleCancelled => 'Запрос отменён.';

  @override
  String get moduleShutdownUnconfirmed =>
      'Не удалось подтвердить завершение работы. Доступ остаётся закрытым.';

  @override
  String get missingReading => 'Нет показания';

  @override
  String get textPreviewLimit => 'Предпросмотр текста, до 16 КиБ.';

  @override
  String get gdsInstall => 'Установка GDS';

  @override
  String get verifiedApplications => 'Проверенные приложения';

  @override
  String get aptObserve => 'Проверка пакетов APT';

  @override
  String get toolchainObserve => 'Проверка инструментов разработки';

  @override
  String get connected => 'Подключено';

  @override
  String get connecting => 'Подключение';

  @override
  String get diagnosticsUnavailable =>
      'Локальная диагностика недоступна. Вход в NDS остаётся доступен.';

  @override
  String get localDiagnosticsOnly =>
      'Локальная диагностика готова. Состояние отправки на сервер показано в разделе «Буфер и оповещения».';

  @override
  String diagnosticsDegraded(
    String rejected,
    String dropped,
    String failures,
    String pending,
  ) {
    return 'Диагностика: отклонено $rejected, потеряно $dropped, ошибок записи $failures, в очереди $pending.';
  }

  @override
  String get startupFailed =>
      'Не удалось запустить NDS. Перезапустите приложение. Если ошибка повторится, проверьте установку и локальную диагностику.';

  @override
  String get diagnosticsHttp => 'Диагностика сети';

  @override
  String get diagnosticsIo => 'Локальные операции';

  @override
  String get devicesTitle => 'Ваши устройства';

  @override
  String get devicesHelp =>
      'У каждой установки свой ключ подписи. Активные зарегистрированные устройства получают доступ к vault.';

  @override
  String get devicesUnverified =>
      'Список устройств ещё не подтверждён свежей проверкой.';

  @override
  String get deviceName => 'Имя устройства';

  @override
  String get enrollDevice => 'Зарегистрировать устройство';

  @override
  String get refreshDevices => 'Обновить устройства';

  @override
  String get thisDevice => 'Эта установка';

  @override
  String get deviceActive => 'Активно';

  @override
  String get deviceRevoked => 'Доступ отозван';

  @override
  String get deviceCreated => 'Зарегистрировано';

  @override
  String get deviceRevokedHelp =>
      'Ключ этого устройства отозван. Он сохранён локально и не может повторно использоваться для регистрации.';

  @override
  String get revokeDevice => 'Отозвать устройство';

  @override
  String revokeDeviceHelp(String name) {
    return 'Отозвать доступ для $name? Эта идентичность не сможет снова зарегистрироваться с тем же ключом.';
  }

  @override
  String get keepDevice => 'Оставить устройство';

  @override
  String get vaultTitle => 'Vault и синхронизация';

  @override
  String get vaultHelp =>
      'Vault открывается автоматически после входа. Изменения синхронизируются через сервер; значения секретов остаются в нативном хранилище и зашифрованных записях.';

  @override
  String get vaultLocked =>
      'Доступ к vault недоступен. Проверьте сессию, устройство и соединение с сервером.';

  @override
  String get vaultUnlocked => 'Защищённый ключ vault доступен.';

  @override
  String vaultCounts(int pending, int conflicts) {
    return 'В очереди: $pending · Конфликты: $conflicts';
  }

  @override
  String get vaultKeyIdentity => 'Идентичность ключа';

  @override
  String get vaultRecordTitle => 'Название записи';

  @override
  String get vaultUsername => 'Имя пользователя или аккаунта';

  @override
  String get vaultSecret => 'Секретное значение';

  @override
  String get vaultSave => 'Зашифровать и отправить на синхронизацию';

  @override
  String vaultRevision(String revision) {
    return 'Ревизия $revision';
  }

  @override
  String get vaultConflict => 'Конфликт не разрешён. Обе версии сохранены.';

  @override
  String get vaultSync => 'Синхронизировать сейчас';

  @override
  String get vaultInputError => 'Проверьте поля записи.';

  @override
  String get vaultProofError => 'Не удалось проверить ключ или запись vault.';

  @override
  String get syncedState => 'Состояние устройств и модулей';

  @override
  String get publishDeviceState =>
      'Отправить состояние устройства, сессии и модулей';

  @override
  String get stateObserved => 'Время наблюдения';

  @override
  String get stateChecked => 'Последняя проверка';

  @override
  String get publishModuleState => 'Отправить наблюдаемое состояние модулей';

  @override
  String get stateHelp =>
      'Это зашифрованные наблюдения подписанных устройств. Время показывает момент наблюдения, а не текущую доступность. Синхронизация не запускает инструменты на другом устройстве.';

  @override
  String get statePublishError =>
      'Состояние не удалось синхронизировать. Проверьте сессию, устройство и подключение к vault.';

  @override
  String get stateDevice => 'Устройство';

  @override
  String get stateDevices => 'Устройства';

  @override
  String get vaultRegistrationRequired =>
      'Существующий ключ vault сначала должен зарегистрировать обновлённый клиент, у которого этот ключ уже есть.';

  @override
  String get syncCatchingUp => 'Получаем изменения с сервера…';

  @override
  String get syncNotObserved => 'Синхронизация ещё не подтверждена.';

  @override
  String get syncCaughtUp =>
      'Изменения получены при последней успешной проверке.';

  @override
  String get passkey => 'Войти с ключом доступа';

  @override
  String get registerPasskey => 'Создать ключ доступа';

  @override
  String get openPasskey => 'Открыть проверку ключа доступа';

  @override
  String get passkeyHelp =>
      'Сравните код в браузере, затем подтвердите ключ доступа. Для создания ключа нужен недавний вход.';

  @override
  String get utilitiesTitle => 'Буфер обмена и предупреждения';

  @override
  String get sharedClipboard => 'Общий буфер';

  @override
  String get alertsTitle => 'Предупреждения';

  @override
  String get clipboardHelp =>
      'Текст шифруется и автоматически синхронизируется с сервером. Отправляйте только нужный текст; чувствительный буфер и URI исключены.';

  @override
  String get sharedText => 'Текст для передачи';

  @override
  String get shareText => 'Передать текст';

  @override
  String get shareNativeClipboard => 'Передать буфер устройства';

  @override
  String get clipboardArchive => 'Выбрать из истории буфера';

  @override
  String clipboardReady(int sequence, int bytes) {
    return 'Серверная версия $sequence · $bytes байт';
  }

  @override
  String get clipboardEmpty => 'Подтверждённого общего текста пока нет';

  @override
  String get copySharedClipboard => 'Скопировать общий текст';

  @override
  String utilityError(String code) {
    return 'Операция недоступна: $code';
  }

  @override
  String telemetryDelivery(
    int pending,
    int delivered,
    int dropped,
    int failures,
  ) {
    return 'В очереди $pending · Принято $delivered · Потеряно $dropped · Неудачных попыток $failures';
  }

  @override
  String get telemetryInitialized =>
      'Доставка клиента активна. Счётчик подтверждает приём сервером.';

  @override
  String get telemetryUnavailable =>
      'Доставка клиента недоступна; локальная диагностика работает.';

  @override
  String get noIncidents => 'Инцидентов нет';

  @override
  String incidentState(String kind, String state, int occurrences) {
    return '$kind · $state · случаев: $occurrences';
  }

  @override
  String get acknowledgeIncident => 'Подтвердить';

  @override
  String get silenceIncident => 'Приглушить на час';

  @override
  String get moreIncidents => 'Следующие инциденты';

  @override
  String syncLastSeen(String time) {
    return 'Последняя полная синхронизация: $time';
  }

  @override
  String get vaultDelete => 'Удалить запись';

  @override
  String vaultDeleteHelp(String title) {
    return 'Удалить «$title» на всех синхронизированных устройствах?';
  }

  @override
  String get vaultResolve => 'Разрешить конфликт';

  @override
  String vaultResolveHelp(String choice) {
    return 'Применить «$choice»? Другие устройства получат это решение после синхронизации.';
  }

  @override
  String get operationBusy =>
      'Выполняется другая операция или данные изменились. Обновите их и повторите попытку.';

  @override
  String vaultEditing(String title) {
    return 'Редактирование: $title. Для сохранения введите секрет.';
  }

  @override
  String get vaultDeleted => 'Удалённая запись';

  @override
  String vaultConflictingTitle(String title) {
    return 'Конфликтующая версия: $title';
  }

  @override
  String get vaultConflictingDelete =>
      'Конфликтующая версия удаляет эту запись.';

  @override
  String get vaultKeepServer => 'Оставить серверную версию';

  @override
  String get vaultUseConflict => 'Выбрать конфликтующую версию';

  @override
  String get vaultCopySecret => 'Скопировать секрет';

  @override
  String get vaultEdit => 'Редактировать';

  @override
  String get vaultNextPage => 'Следующие записи';

  @override
  String get vaultFirstPage => 'Первые записи';

  @override
  String get passkeysTitle => 'Ваши ключи доступа';

  @override
  String get passkeysHelp =>
      'Удаление ключа завершает созданные с ним сессии. Для удаления нужно войти в систему в течение последних пяти минут.';

  @override
  String get passkeysEmpty => 'Ключи доступа не зарегистрированы.';

  @override
  String passkeyCreated(String time) {
    return 'Создан: $time';
  }

  @override
  String get passkeyRemove => 'Удалить ключ доступа';

  @override
  String passkeyRemoveHelp(String id) {
    return 'Удалить ключ $id? Сессии, созданные этим ключом, будут отозваны, возможно включая текущую.';
  }

  @override
  String get passkeyFreshLogin =>
      'Войдите заново перед изменением ключей доступа.';

  @override
  String get keepPasskey => 'Оставить ключ';

  @override
  String get passkeyMethod => 'Ключ доступа';

  @override
  String get actionCancel => 'Отмена';

  @override
  String get exportUnknown =>
      'Политика отправки на сервер ещё не подтверждена.';

  @override
  String get exportEnabled => 'Отправка на сервер разрешена.';

  @override
  String get exportDisabled => 'Оператор отключил отправку на сервер.';

  @override
  String get exportPaused =>
      'Отправка приостановлена. Вернитесь в приложение, чтобы продолжить.';

  @override
  String get eventsLabel => 'События';

  @override
  String get spansLabel => 'Трассировки операций';

  @override
  String exportLastSeen(String time) {
    return 'Последний приём сервером: $time';
  }

  @override
  String incidentsObserved(String time) {
    return 'Состояние оповещений проверено: $time';
  }

  @override
  String get incidentsUnavailable =>
      'Не удалось обновить оповещения. Показанные данные могут быть устаревшими.';

  @override
  String get clipboardUnavailable =>
      'Не удалось обновить состояние общего буфера.';

  @override
  String get clipboardRejected =>
      'Этот элемент буфера помечен как приватный, не поддерживается или слишком велик. Выберите обычный текст до 8 КиБ.';

  @override
  String get clipboardCopied => 'Скопировано в буфер устройства.';

  @override
  String get offlineAuthorized =>
      'Офлайн-доступ использует последнюю проверку сессии и устройства. Изменения остаются на этом устройстве до синхронизации.';

  @override
  String get storageFull =>
      'Локальное хранилище заполнено. Изменение не сохранено.';

  @override
  String get logoutPending =>
      'Локальный выход выполнен. Выход на сервере завершится после подключения.';

  @override
  String get recentAuthenticationRequired =>
      'Для управления ключами доступа войдите снова. Текущая сессия остаётся действующей.';

  @override
  String get browserPrivacy =>
      'Сессия хранится в защищённой cookie. Ключи vault зашифрованы в профиле браузера; сервер передаёт их после подтверждённого входа и регистрации устройства.';

  @override
  String get browserVaultHelp =>
      'Vault открывается автоматически после входа. Изменения хранятся в браузере в зашифрованном виде и синхронизируются через сервер. Сохраняйте данные сайта, пока все изменения не синхронизированы.';

  @override
  String get browserStoragePersistent =>
      'Браузер разрешил постоянное хранение. Очистка данных сайта всё равно удалит локальные изменения.';

  @override
  String get browserStorageEvictable =>
      'При нехватке места браузер может удалить данные сайта. Несинхронизированные изменения будут потеряны.';

  @override
  String get vaultRecordPending => 'Локальное изменение ожидает синхронизации';

  @override
  String get telemetryPersistenceReady =>
      'Запись журналов в хранилище браузера подтверждена.';

  @override
  String get telemetryPersistenceUnavailable =>
      'Успешная запись журналов в постоянное хранилище браузера не подтверждена. Несохранённые события могут потеряться при закрытии страницы.';

  @override
  String get vaultStorageUnavailable =>
      'Локальное зашифрованное хранилище недоступно. Проверьте поддержку хранилища и разрешения браузера или устройства.';

  @override
  String get vaultDependencyUnavailable =>
      'Vault не смог получить доступ к необходимому локальному хранилищу или серверным сервисам.';

  @override
  String get serverCapacityExhausted =>
      'На сервере достигнут лимит хранения. Зашифрованные изменения остаются на этом устройстве. NDS проверяет возможность безопасного повтора; повтор можно запустить и вручную.';

  @override
  String get catalogTitle => 'Репозитории GitHub';

  @override
  String get catalogHelp =>
      'Выберите запись vault с токеном GitHub, чтобы обновить сохранённый каталог. Каждый запрос читает одну страницу, доступную этим учётным данным.';

  @override
  String get catalogCredential => 'Учётные данные из сейфа';

  @override
  String get catalogRefreshCredentials => 'Обновить записи сейфа';

  @override
  String get catalogMoreCredentials => 'Следующие записи сейфа';

  @override
  String get catalogNoCredentials =>
      'На этой странице сейфа нет доступных записей. Откройте сейф, чтобы добавить запись или разрешить конфликт.';

  @override
  String get catalogRead => 'Первая страница';

  @override
  String get catalogNextPage => 'Следующая страница репозиториев';

  @override
  String get catalogChooseCredential => 'Выберите актуальную запись сейфа.';

  @override
  String get catalogSignIn =>
      'Войдите в аккаунт и подтвердите это устройство для просмотра репозиториев.';

  @override
  String get catalogCredentialChanged =>
      'Запись сейфа изменилась. Обновите записи и выберите её повторно.';

  @override
  String get catalogInvalidResponse =>
      'Не удалось проверить данные репозиториев.';

  @override
  String get catalogProviderFailed =>
      'GitHub не вернул эту страницу. Проверьте доступ учётных данных и повторите запрос.';

  @override
  String get catalogUnavailable => 'Каталог репозиториев недоступен.';

  @override
  String get catalogObservationUnknown => 'Время наблюдения недоступно.';

  @override
  String catalogObserved(String time) {
    return 'Время наблюдения: $time';
  }

  @override
  String catalogPrincipal(String login) {
    return 'Аккаунт GitHub: $login';
  }

  @override
  String get catalogPageScope =>
      'Показаны только репозитории, доступные выбранным учётным данным. Каждый запрос заменяет эту страницу.';

  @override
  String get catalogEmptyPage => 'На этой странице нет репозиториев.';

  @override
  String get catalogOrganization => 'Организация';

  @override
  String get catalogPersonal => 'Личный аккаунт';

  @override
  String get catalogPublic => 'Публичный';

  @override
  String get catalogPrivate => 'Приватный';

  @override
  String get catalogInternal => 'Внутренний';

  @override
  String get catalogArchived => 'Архивный';

  @override
  String get catalogHasMore =>
      'Следующая страница содержит дополнительные репозитории.';

  @override
  String get catalogAccessEnd =>
      'Конец списка, доступного этим учётным данным. При другом доступе список может содержать больше репозиториев.';

  @override
  String get catalogLimitReached =>
      'Достигнут локальный лимит страниц. GitHub сообщает о дополнительных репозиториях; этот список неполный.';

  @override
  String get catalogCancelled => 'Запрос репозиториев отменён.';

  @override
  String get catalogTimeout =>
      'Время ожидания запроса истекло. Повторите запрос.';

  @override
  String get catalogPaused =>
      'Запросы репозиториев приостановлены, пока приложение находится в фоне.';

  @override
  String get screenView => 'Просмотреть экран';

  @override
  String get screenEnded => 'Просмотр экрана завершён.';

  @override
  String get screenOpening => 'Ожидание доступа к экрану…';

  @override
  String get screenUnavailable => 'Экран недоступен.';

  @override
  String get screenStop => 'Завершить просмотр';

  @override
  String get screenRequests => 'Запросы просмотра экрана';

  @override
  String get screenRequestsHelp =>
      'Обновите запросы от устройств, которым разрешён доступ по правилам компании. Передача экрана начнётся только после вашего разрешения.';

  @override
  String get screenConsent =>
      'Разрешить этому устройству просмотр экрана в запрошенной сессии?';

  @override
  String get screenAllow => 'Разрешить просмотр экрана';

  @override
  String get screenAccessAllowed => 'Доступ к экрану разрешён для этой сессии.';

  @override
  String get screenStopSharing => 'Прекратить передачу';

  @override
  String get catalogSaved => 'Сохранённый каталог';

  @override
  String get catalogProvider => 'Обновление из GitHub';

  @override
  String get catalogSavedHelp =>
      'Зашифрованные сведения, сохранённые в NDS. Даты показывают время наблюдения, а не текущую актуальность или доступ к GitHub.';

  @override
  String get catalogReadSaved => 'Обновить сохранённую страницу';

  @override
  String get catalogSavedNext => 'Следующая сохранённая страница';

  @override
  String get catalogReadSavedAgain =>
      'Обновите сохранённый каталог и повторите попытку.';

  @override
  String catalogMissingOwner(String id) {
    return 'Владелец $id: сведения ещё не синхронизированы.';
  }

  @override
  String get catalogCapacityReached =>
      'Каталог обновлён не полностью: достигнут лимит хранилища синхронизации. Сохранённые сведения остаются доступны.';

  @override
  String get companyTitle => 'Компания';

  @override
  String get companyPrivacy =>
      'Управление компанией доступно онлайн. Личный Vault, секреты и репозитории доступны только владельцу.';

  @override
  String get companyAdmin => 'Администратор';

  @override
  String get companyUser => 'Пользователь';

  @override
  String get companyCompleted =>
      'Сервер подтвердил изменение. Обновите данные, чтобы увидеть текущее состояние.';

  @override
  String get companyLastAdmin =>
      'Нельзя удалить последнего активного администратора или понизить его роль.';

  @override
  String get companyConflict =>
      'Данные участника изменились после загрузки. Проверьте текущее состояние.';

  @override
  String get companyExpired =>
      'Срок команды истёк. Проверьте текущее состояние перед новым действием.';

  @override
  String get companyInvitationUnavailable =>
      'Это приглашение или получатель больше недоступны для действия.';

  @override
  String get companyDenied => 'Ваша текущая роль не разрешает это действие.';

  @override
  String get companyInvalid => 'Проверьте email и выбранное действие.';

  @override
  String get companyPending =>
      'Исходная команда сохранена для повтора. При сбое связи сервер мог её выполнить. Проверьте состояние перед новой командой. Закрытие экрана не повторяет отправку.';

  @override
  String get companyRetry => 'Повторить исходную команду';

  @override
  String get companyReview => 'Проверить состояние и завершить эту команду';

  @override
  String get companyMembers => 'Участники';

  @override
  String get companyActive => 'Активен';

  @override
  String get companyRemoved => 'Удалён';

  @override
  String get companyRemove => 'Удалить участника';

  @override
  String get companyChangeRole => 'Изменить роль';

  @override
  String companyChangeHelp(String member) {
    return 'Применить изменение к $member? Удаление участника завершает его активные сессии; личные данные сохраняются.';
  }

  @override
  String get companyConfirm => 'Подтвердить';

  @override
  String get companyNextMembers => 'Следующие участники';

  @override
  String get companyInvite => 'Пригласить по email';

  @override
  String get companyDeliveryHelp =>
      'Создание приглашения, принятие почтовым сервисом и вступление в компанию — отдельные шаги. Ссылка действует семь дней.';

  @override
  String get companyInvitePending => 'Ожидает принятия';

  @override
  String get companyJoined => 'Вступил';

  @override
  String get companyInviteExpired => 'Истекло';

  @override
  String get companyInviteRevoked => 'Отозвано';

  @override
  String get companyDeliveryPending => 'Письмо: ожидается результат отправки';

  @override
  String get companyDeliveryAccepted => 'Письмо: принято почтовым сервисом';

  @override
  String get companyDeliveryFailed => 'Письмо: не отправлено';

  @override
  String get companyDeliveryUnknown => 'Письмо: результат отправки неизвестен';

  @override
  String get companyRevoke => 'Отозвать приглашение';

  @override
  String get companyNextInvitations => 'Следующие приглашения';

  @override
  String get invitationImport => 'Открыть приглашение';

  @override
  String get invitationLink => 'Ссылка-приглашение';

  @override
  String get invitationVerify =>
      'Приглашение загружено. Укажите email получателя и подтвердите код из письма. Сама ссылка не выполняет вход.';

  @override
  String get invitationSignOut =>
      'Получено приглашение. Сначала выйдите из текущего аккаунта, затем подтвердите email получателя.';

  @override
  String get passkeyAccountHint =>
      'Для входа с ключом доступа укажите email вашего аккаунта NDS. Если участников несколько, необходимо выбрать аккаунт.';

  @override
  String get screenTargets => 'Устройства для просмотра экрана';

  @override
  String get screenTargetsEmpty => 'Нет активных устройств.';

  @override
  String get screenNotRegistered =>
      'Сначала откройте общий доступ к экрану на этом устройстве.';

  @override
  String get screenTargetsFailed =>
      'Не удалось загрузить устройства. Обновите список, чтобы проверить доступ и соединение.';

  @override
  String get moduleManagementHelp =>
      'Включайте модули и выбирайте их разрешения NDS для этого устройства. Для доступа к возможностям системы или браузера по-прежнему требуется их собственное разрешение.';

  @override
  String get moduleEnabled => 'Модуль включён';

  @override
  String get modulePermissions => 'Разрешения NDS';

  @override
  String get moduleSave => 'Сохранить настройки модуля';

  @override
  String get moduleSaved =>
      'Настройки сохранены на этом устройстве; синхронизация выполняется отдельно.';

  @override
  String get moduleDisabled =>
      'Включите модуль, чтобы использовать его возможности.';

  @override
  String get modulePermissionRequired =>
      'Выберите необходимые разрешения NDS перед использованием этой возможности.';

  @override
  String get moduleConfigurationConflict =>
      'Конфликт настроек модуля. До его разрешения доступ не предоставляется.';

  @override
  String get moduleDeviceRequired =>
      'Войдите и зарегистрируйте это устройство для управления модулями.';

  @override
  String get moduleNotInstalled =>
      'Нужный локальный инструмент не установлен или не найден.';

  @override
  String get moduleInstalled => 'Доступен на этом устройстве';

  @override
  String get moduleInstallationUnknown => 'Доступность ещё не проверена';

  @override
  String get moduleMetadataError =>
      'Не удалось обновить сведения о модулях. Результат последней операции не изменён.';

  @override
  String get permissionReadOnly => 'Чтение локальных сведений';

  @override
  String get permissionCredentialStore =>
      'Использование защищённых учётных данных';

  @override
  String get permissionProcessExecution =>
      'Запуск заявленного локального инструмента';

  @override
  String get permissionLocalNetwork => 'Соединение с заявленным сервисом';

  @override
  String get permissionUserService => 'Доступ к сервису текущего пользователя';

  @override
  String get permissionClipboard => 'Использование буфера обмена';

  @override
  String get moduleSettingsChanged =>
      'Настройки модуля изменились. Обновите форму перед сохранением своего выбора.';

  @override
  String get moduleReloadDraft => 'Обновить настройки';

  @override
  String get catalogSourceHelp =>
      'Выбор сохраняется и синхронизируется в аккаунте. Разрешения GDS включаются отдельно на каждом устройстве.';

  @override
  String get catalogClearSource => 'Сбросить выбор GitHub';

  @override
  String get moduleStateQueued =>
      'Локальные состояния поставлены в очередь синхронизации.';

  @override
  String get permissionGitWrite =>
      'Подключать репозитории и синхронизировать Git-коммиты';

  @override
  String get permissionBrowserControl => 'Управление вкладками браузера агента';

  @override
  String get browserControlScope =>
      'Разрешает агентам автоматизацию вкладок в группах NDS. На остальные вкладки браузера это разрешение не распространяется.';

  @override
  String get repositoryConnect => 'Подключить репозиторий';

  @override
  String repositoryConnected(String path) {
    return 'Репозиторий подключён: $path';
  }

  @override
  String repositoryRegistrationPending(String path) {
    return 'Репозиторий находится в $path. Повторите подключение, чтобы завершить регистрацию в NDS.';
  }

  @override
  String get repositoryIntentQueued =>
      'Намерение подключения поставлено в очередь синхронизации NDS.';

  @override
  String get repositoryPathOccupied =>
      'Это место уже занято. Существующие файлы сохранены.';

  @override
  String get repositoryDifferentInstallation =>
      'Репозиторий привязан к другому аккаунту или установке NDS.';

  @override
  String get repositoryRelocationRequired =>
      'Сохранённая привязка использует другое расположение. Сначала проверьте перенос репозитория.';

  @override
  String get repositoryWrongRemote =>
      'Git origin или сохранённый идентификатор не соответствует выбранному репозиторию.';

  @override
  String get repositoryUnsupported =>
      'Локальное подключение репозиториев не поддерживается на этом устройстве.';

  @override
  String get repositorySyncRunning => 'Синхронизация существующих коммитов…';

  @override
  String get repositorySyncCurrent => 'Коммиты синхронизированы';

  @override
  String get repositorySyncPublished => 'Существующие коммиты отправлены';

  @override
  String get repositorySyncFastForwarded => 'Checkout обновлён без слияния';

  @override
  String get repositorySyncDirty =>
      'Незакоммиченные изменения сохранены; синхронизация этого checkout отложена';

  @override
  String get repositorySyncDiverged => 'Истории разошлись; изменения сохранены';

  @override
  String get repositorySyncRefused =>
      'В синхронизации отказано; существующая работа сохранена';

  @override
  String get appUpdateTitle => 'Обновления приложения NDS';

  @override
  String get appUpdateCheck => 'Проверить обновления';

  @override
  String get appUpdateDownload => 'Скачать обновление';

  @override
  String get appUpdateCancel => 'Отменить загрузку';

  @override
  String get appUpdateIdle => 'Проверьте наличие обновления приложения.';

  @override
  String get appUpdateChecking => 'Проверяем обновление…';

  @override
  String get appUpdateAvailable =>
      'Проверенное обновление доступно для загрузки.';

  @override
  String get appUpdateDownloading => 'Загружаем обновление…';

  @override
  String get appUpdateStaged =>
      'Загрузка проверена и сохранена. Установка в этой сборке пока недоступна.';

  @override
  String get appUpdateUnsupported =>
      'Обновление приложения на этой платформе не поддерживается.';

  @override
  String get appUpdateNotConfigured =>
      'Для этой сборки обновление приложения не настроено.';

  @override
  String get appUpdateBusy =>
      'Уже выполняется другая операция обновления. Дождитесь её завершения.';

  @override
  String get appUpdateCancelled => 'Загрузка отменена.';

  @override
  String get appUpdateTimeout =>
      'Время операции обновления истекло. Попробуйте ещё раз.';

  @override
  String get appUpdateNetwork =>
      'Не удалось получить обновление из разрешённого источника.';

  @override
  String get appUpdateTooLarge => 'Размер загрузки не совпадает с ожидаемым.';

  @override
  String get appUpdateSignature =>
      'Не удалось проверить издателя или подписанное описание обновления.';

  @override
  String get appUpdateInvalid =>
      'Обновление недействительно или загрузка не завершена.';

  @override
  String get appUpdateSource =>
      'Обновление не соответствует разрешённому кандидату этой сборки.';

  @override
  String get appUpdateCompatibility =>
      'Совместимость данных неизвестна или не поддерживается.';

  @override
  String get appUpdateNotNewer =>
      'Версия кандидата не новее установленного приложения.';

  @override
  String get appUpdateStorage =>
      'Не удалось безопасно сохранить или восстановить обновление.';

  @override
  String appUpdateVersion(String version) {
    return 'Кандидат $version';
  }

  @override
  String appUpdateProgress(String downloaded, String total) {
    return '$downloaded / $total байт';
  }

  @override
  String get browserAutomation => 'Автоматизация браузера';

  @override
  String get networkCache => 'Сетевой кеш';

  @override
  String get browserOwnerSwitch => 'Переключатель владельца браузера';

  @override
  String get browserHostStatus => 'Браузер';

  @override
  String get browserExtensionStatus => 'Расширение';

  @override
  String get browserSessionCount => 'Сессии браузера';

  @override
  String get browserSessionScope =>
      'Здесь показаны сессии хоста браузера. Этот список не даёт управление сессиями другого агента.';

  @override
  String browserTabCount(String count) {
    return 'Вкладок: $count';
  }

  @override
  String get browserAllowWork => 'Разрешить работу этого агента в браузере';

  @override
  String get cacheConfiguredPort => 'Заданный локальный порт';

  @override
  String get cacheReachability => 'Доступность кеша';

  @override
  String get cacheProbeReady => 'Отвечает';

  @override
  String get cacheProbeUnavailable => 'Не отвечает';

  @override
  String get cacheManagedScope =>
      'Управляемые настройки показаны отдельно от доступности кеша. Это не подтверждает, что запущенные инструменты используют заданный порт.';

  @override
  String get cacheManagedPresent => 'Управляемые настройки найдены';

  @override
  String get cacheManagedAbsent => 'Управляемые настройки не найдены';

  @override
  String get cacheAssemblyInvalid =>
      'Настройка кеша для этого приложения недействительна.';

  @override
  String get browserSwitchEnabled => 'Включён';

  @override
  String get browserSwitchDisabled => 'Выключен';
}
