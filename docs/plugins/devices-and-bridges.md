# Устройства, bridges и специализированные runtimes

Ранее записанные направления собраны здесь вместе с новыми идеями; [реестр](README.md) различает обязательные этапы и кандидатов без срока. Статус реальных packages — проектирование. Подтверждённые descriptors/fixtures и физически проверенное оборудование не смешиваются.

<a id="plg-026"></a>
## PLG-026 — Mirabox N4 Pro / Stream Deck-подобные панели

DeviceProvider предоставляет keys/encoders/touch/display/light/haptic только по подтверждённым model/firmware capabilities. N4 Pro — первый реальный target; другие панели получают собственные adapters. Full surface scene и hit regions, frames/ACK/resync принадлежат общим contracts. Task board, palette и game не знают HID reports. Приёмка: hotplug, frame drop/resync, unsupported touch release и разные размеры экранов. [N4 Pro module](../architecture/modules/n4pro-plugin.md), Фазы 8/9.

<a id="plg-027"></a>
## PLG-027 — Gamepad/HID и другие контроллеры

Независимый input family проверяет analog axes, buttons, calibration/units, haptic при наличии. Рули/force feedback — более позднее расширение с отдельными safety/ownership budgets. Общие actions/scenes работают без special-case «кнопка только Stream Deck». Приёмка: дрейф оси, reliable encoder deltas, neutral rearm, отключение и несколько одинаковых устройств. [Gamepad module](../architecture/modules/gamepad-plugin.md), Фаза 10; Device Studio Фазы 15 переиспользует descriptors.

<a id="plg-028"></a>
## PLG-028 — Клавиатуры, RGB и OpenRGB

Обычная input keyboard, analog/magnetic travel sensor и RGB controller могут быть разными contributions/packages. Provider сообщает geometry и key↔LED mapping; wave/press→dim→cooldown→fade исполняет общий lighting engine. OpenRGB bridge либо прямой transport выбирается по evidence, не два одновременных writers одного устройства. Приёмка: irregular layout, shared LEDs, input без RGB, geometry отсутствует, repeated press и смена слоя. [Lighting](../contracts/lighting.md), [сценарий](../product/scenarios.md#клавиатура--волна-света-затемнение-и-восстановление).

<a id="plg-029"></a>
## PLG-029 — Smart keyboard и onboard profiles

Capability для чтения/выбора volatile preset и позже typed persistent programming. Содержимое program slot, limits/endurance, readback, plan/diff/consent и recovery отличаются от host scenes. Host automation остаётся доступной устройствам без onboard API. Приёмка: эквивалентность timing/retrigger/cleanup host/device, interrupted write и unsupported slots. Firmware flashing исключён. [Provisioning](../contracts/capabilities.md#provisioning-capability), после v1.

<a id="plg-030"></a>
## PLG-030 — OpenAction/Elgato

Отдельный compatibility host переводит legacy actions/settings/feedback, явно показывает уровни C0–C3 и limitations results/jobs. PI/custom view не получает privileged host bridge. Native replacement migration сохраняет rollback. Приёмка только по version-pinned packages; legacy delivery не становится typed result. [Compatibility](../contracts/compatibility.md), Фаза 14.

<a id="plg-031"></a>
## PLG-031 — Bitfocus Companion

Bridge actions/variables/feedback через публичные interfaces; version-pinned internal adapter допускается только изолированно и с fixtures. Не переносит workflow ownership в скрытый второй scheduler. Failure не ломает native providers. [Ранее выбранные границы](../contracts/compatibility.md#будущий-bitfocus-companion-bridge), после v1.

<a id="plg-032"></a>
## PLG-032 — Remote panels/agents

Steam Deck/Android/touch panels, cable/Wi-Fi transport, remote inputs/displays; camera/microphone/speaker только по отдельным session grants. Shared endpoint identity дополняется remote node/session, clock uncertainty и reconnect policy. Pairing/revoke, bounded network queues и отсутствующий peer не создают stuck outputs. [Remote Agent](../architecture/modules/remote-agent.md), после v1; distributed active-active workspace не обещан.

<a id="plg-033"></a>
## PLG-033 — Audio/DSP и виртуальный микрофон

Mic source → filters/EQ/noise suppression → virtual sink. Control plugins меняют параметры/читают meters; buffers остаются в specialized media plane. CLAP, optional GStreamer/VST bridges — кандидаты с отдельными лицензиями и latency/backend gate. File capture PLG-013 не доказывает low-latency streaming. Приёмка: rate/channel negotiation, clock, hotplug, xruns, crash и bypass только при рабочем совместимом пути. [Media contract](../contracts/media.md), отдельная программа после v1.

<a id="plg-034"></a>
## PLG-034 — OBS

Ранее упомянутый кандидат Action/StateProvider: scene/source/recording operations и observed state через versioned OBS adapter. Общие typed results, reconnect, account/endpoint grants и unknown write outcome; UI — PLG-022. Наличие OBS в примерах plugin model не является обязательством текущей фазы. Перед реализацией — API/version spike и evidence фактического перехода состояния.

<a id="plg-035"></a>
## PLG-035 — MIDI

Ранее упомянутый ProtocolBridge: input/output ports, channel/controller/note mapping и device identity. Note on/off сохраняют edge ordering и owner cleanup, feedback не образует бесконечный loop. Точная версия MIDI и transport выбираются отдельным spike; mapping не добавляется в core. Срок не назначен.

<a id="plg-036"></a>
## PLG-036 — OSC

Ранее упомянутый ProtocolBridge: address/type mapping, allowed endpoints, events/state и outbound commands. UDP loss/reorder и bundle timing отражаются в descriptor/policy; сетевой send не считается внешним completion. Same result/limits/loop contracts, без произвольного RPC из UI. Срок не назначен.
