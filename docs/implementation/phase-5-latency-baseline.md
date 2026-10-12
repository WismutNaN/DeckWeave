# Phase 5 replay и latency baseline

Дата измерения: 2026-10-12. Это engineering baseline debug-сборки на текущем Windows host, а не SLA и не сравнение платформ.

## Что измеряется

`SimulationRunner` исполняет тот же `CompiledExecutionPlan` и `WorkspaceScheduler`, что live Host, но создаёт новое состояние для каждого запуска и заменяет runtime dispatcher bounded recorder без live capabilities.

| Метрика | Граница |
|---|---|
| ingress → dispatch | От начала admission принятого ingress до записи каждого порождённого intent |
| queue wait | От начала admission до начала соответствующего `WorkspaceScheduler::step` |
| scheduler step | Только вызов production `WorkspaceScheduler::step` |
| provider completion | Разница fake-clock ticks между `Invoke` и terminal/rejected fixture event |

Fixture содержит 250 последовательных пар signal/terminal, то есть 500 ingress и 750 intents (`Invoke`, `Feedback`, `CleanupRun`). Два запуска сравнивают final state, intents и semantic trace; wall-clock latency намеренно не участвует в проверке детерминизма.

Команда воспроизведения:

```powershell
$env:DECKWEAVE_PRINT_LATENCY='1'
cargo test -p deckweave-automation --test simulation load_fixture_is_repeatable_and_reports_percentile_baselines -- --nocapture
Remove-Item Env:DECKWEAVE_PRINT_LATENCY
```

## Зафиксированный debug run

| Метрика | Samples | p50 | p95 | p99 |
|---|---:|---:|---:|---:|
| ingress → dispatch | 750 | 80.6 µs | 135.9 µs | 142.6 µs |
| queue wait | 500 | 1.2 µs | 1.6 µs | 2.2 µs |
| scheduler step | 500 | 76.9 µs | 132.6 µs | 138.7 µs |
| provider completion | 250 | 1 tick | 1 tick | 1 tick |

На этом fixture основная локальная стоимость находится внутри scheduler transition, а не queue admission. Оптимизация до появления release benchmark и реального workload не требуется.

## Bounds и failure evidence

- Число fixture events проверяется до запуска и не превышает `maximum_trace_entries`.
- Trace хранится в bounded ring и сообщает `dropped_entries`.
- Recording dispatcher не блокирует scheduler, ограничен тем же policy budget и при насыщении заменяет последний slot на `CleanupAll`, выставляет `Overloaded` и считает dropped intents.
- Ingress overflow использует отдельный cleanup path, которому не требуется место в заполненной scheduler queue.
- Pending/completed/run state ограничены `GraphLimits`; после terminal load-run нет pending invocations или active runs.
- Повторный `run` создаёт новый scheduler: состояние, очереди и recorder не переносятся между запусками.
- Missing terminal outcome возвращает `Inconclusive`; live effect не вызывается и автоматически не повторяется.

Default live trace и simulation trace не содержат payload. Для точного replay пользователь должен явно предоставить/разрешить versioned fixture с входными значениями и outcomes; долговременный opt-in recorder и его UI относятся к Фазе 10.
