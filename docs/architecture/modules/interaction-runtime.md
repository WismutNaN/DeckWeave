# Модуль: Interaction Runtime

**Контекст:** короткие host-owned desktop interactions, возвращающие typed result в конкретный automation invocation.  
**Расположение:** contracts и session state machine в `crates/deckweave-interaction/`, будущий presentation adapter — в desktop shell, custom web views — в lazy isolated runtime.

## Ответственность

- открыть, обновить и завершить `InteractionSession`;
- связать request/result с `InvocationId` и `InteractionSessionId`;
- host-owned window, focus, placement, keyboard navigation и accessibility;
- cancel, Escape, timeout, provider loss и concurrency policy;
- schema-native list/form/preview views и scoped sandboxed extension view.

## Публичная граница

| Символ | Роль |
|---|---|
| `InteractionRequest` | тип view, query schema, initial state и policy |
| `InteractionSession` | identity, owner invocation, deadline и status |
| `InteractionUpdate` | bounded page/status/validation delta |
| `InteractionResult` | typed selected value или explicit cancel/error |
| `InteractionPresenter` | application port host-owned UI |

## Инварианты

- terminal result доставляется только owner invocation и не дублируется;
- закрытие окна является explicit cancel, а не success с пустым значением;
- два concurrent sessions не смешивают query, selection или result;
- provider не владеет окном и не вызывает automation continuation;
- custom view получает только instance-scoped bridge и объявленные grants;
- runtime не запускается, пока ни одна interaction не требует custom web content.

## Пример Zotero

Keyboard event запускает automation action `zotero.search`. Action открывает search interaction, provider выдаёт страницы `ItemRef`, пользователь выбирает результат, а terminal result передаётся следующему `zotero.open_attachment` step. Zotero-specific query/PDF logic остаётся в plugin, окно и session semantics — в Interaction Runtime.

R4 реализует bounded pages, selected/cancelled/timed-out terminal outcomes, duplicate-terminal rejection, owner isolation и presenter/provider ports с fakes. Реальные окна, Zotero provider и custom web content намеренно не подключены.

Это начальный scaffold: лимит количества items ещё не ограничивает bytes/pages/sessions, строка result schema не выполняет validation, а хранение terminal sessions пока не имеет eviction. До presenter Фазы 13 нужны application service с проверкой schema и owner grants, query generation для отбрасывания устаревшего поиска, provider/activation fencing, monotonic deadline и bounded terminal retention. Wall-clock deadline в wire DTO переводится в host monotonic budget при приёме; пользовательская смена часов не продлевает session.

Постоянные notes/dashboard overlays получают отдельный `DesktopViewSession` в presentation boundary, не бессрочную InteractionSession. Shared host window adapter может обслуживать оба lifecycle, но types/owners/retention разные. Provider startup и search query идут независимо: окно принимает текст до readiness, старые query generations отбрасываются. Целевые правила и этапы: [Presentation](../../contracts/presentation.md), [pre-6 gate](../../implementation/pre-phase-6.md).

## Намеренно не обрабатывает

- surface rendering на N4 Pro;
- property editing plugin settings;
- произвольную browser navigation;
- domain API Zotero или другой интеграции.
