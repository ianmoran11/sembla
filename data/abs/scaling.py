"""Deterministic scale reduction with exact national and two-way margins.

A single Hare--Niemeyer pass over all cells preserves only the national total;
it does not generally preserve state and age margins.  ``scale_with_margins``
therefore applies largest remainder to the two margin sets and then solves the
remaining cell increments as a deterministic minimum-cost bipartite flow.  The
result preserves the national, state and age targets exactly while choosing the
largest feasible cell remainders.

Individual cells can move to either adjacent integer around their exact quota.
That perturbation matters at ``hundredth`` scale: Northern Territory and
Australian Capital Territory single-year cells are frequently zero or one and
must be reported rather than smoothed.
"""

from __future__ import annotations

from dataclasses import dataclass
import heapq
from typing import Callable, Hashable, Mapping, TypeVar


Cell = TypeVar("Cell", bound=Hashable)
RowMargin = TypeVar("RowMargin", bound=Hashable)
ColumnMargin = TypeVar("ColumnMargin", bound=Hashable)


@dataclass(frozen=True)
class ScaledCounts:
    """One constrained apportionment and its exact integer diagnostics.

    ``errors`` are stored in source-person units: for a cell ``key``, the value
    is ``counts[key] * divisor - source[key]``.  Dividing by ``divisor`` gives
    the signed scaled-person rounding error without introducing floating-point
    behaviour into the reproducibility boundary.
    """

    counts: dict[Cell, int]
    total: int
    row_margins: dict[Hashable, int]
    column_margins: dict[Hashable, int]
    errors: dict[Cell, int]
    divisor: int


@dataclass
class _Edge:
    to: int
    reverse: int
    capacity: int
    cost: int


def _validate_counts(counts: Mapping[Cell, int], divisor: int) -> list[Cell]:
    if isinstance(divisor, bool) or not isinstance(divisor, int) or divisor <= 0:
        raise ValueError("divisor must be a positive integer")
    keys = sorted(counts)
    for key in keys:
        value = counts[key]
        if isinstance(value, bool) or not isinstance(value, int) or value < 0:
            raise ValueError(f"count for {key!r} must be a non-negative integer")
    return keys


def rounded_total(total: int, divisor: int) -> int:
    """Return nearest-integer ``total / divisor``, with exact halves rounded up."""
    if isinstance(total, bool) or not isinstance(total, int) or total < 0:
        raise ValueError("total must be a non-negative integer")
    if isinstance(divisor, bool) or not isinstance(divisor, int) or divisor <= 0:
        raise ValueError("divisor must be a positive integer")
    quotient, remainder = divmod(total, divisor)
    return quotient + (2 * remainder >= divisor)


def largest_remainder(
    counts: Mapping[Cell, int],
    divisor: int,
    *,
    target: int | None = None,
) -> dict[Cell, int]:
    """Scale one margin by Hare--Niemeyer with a sorted-key tie break."""
    keys = _validate_counts(counts, divisor)
    if target is None:
        target = rounded_total(sum(counts.values()), divisor)
    if isinstance(target, bool) or not isinstance(target, int) or target < 0:
        raise ValueError("target must be a non-negative integer")

    result = {key: counts[key] // divisor for key in keys}
    increments = target - sum(result.values())
    ranked = sorted(keys, key=lambda key: (-(counts[key] % divisor), key))
    eligible = [key for key in ranked if counts[key] % divisor]
    if increments < 0 or increments > len(eligible):
        raise ValueError(
            f"target {target} cannot be reached by floor-or-ceiling rounding"
        )
    for key in eligible[:increments]:
        result[key] += 1
    return result


def apportion_with_margins(
    weights: Mapping[Cell, int],
    target: int,
    *,
    row_of: Callable[[Cell], RowMargin],
    column_of: Callable[[Cell], ColumnMargin],
) -> ScaledCounts:
    """Apportion ``target`` units by weights with exact two-way margins.

    Each quota is ``weights[key] * target / sum(weights)``.  Multiplying the
    integer numerators first lets ``scale_with_margins`` perform the same exact
    floor/ceiling optimisation without floating point.
    """
    keys = _validate_counts(weights, 1)
    if isinstance(target, bool) or not isinstance(target, int) or target < 0:
        raise ValueError("target must be a non-negative integer")
    total_weight = sum(weights.values())
    if total_weight == 0:
        if target:
            raise ValueError("positive target cannot be apportioned from zero weights")
        return ScaledCounts(
            counts={key: 0 for key in keys},
            total=0,
            row_margins={row_of(key): 0 for key in keys},
            column_margins={column_of(key): 0 for key in keys},
            errors={key: 0 for key in keys},
            divisor=1,
        )
    numerators = {key: weights[key] * target for key in keys}
    return scale_with_margins(
        numerators,
        total_weight,
        row_of=row_of,
        column_of=column_of,
    )


def _add_edge(graph: list[list[_Edge]], source: int, target: int,
              capacity: int, cost: int) -> int:
    forward_index = len(graph[source])
    reverse_index = len(graph[target])
    graph[source].append(_Edge(target, reverse_index, capacity, cost))
    graph[target].append(_Edge(source, forward_index, 0, -cost))
    return forward_index


def _minimum_cost_cell_increments(
    keys: list[Cell],
    counts: Mapping[Cell, int],
    divisor: int,
    row_of: Callable[[Cell], RowMargin],
    column_of: Callable[[Cell], ColumnMargin],
    row_deficits: Mapping[RowMargin, int],
    column_deficits: Mapping[ColumnMargin, int],
) -> set[Cell]:
    """Select floor-to-ceiling increments satisfying both margin deficits."""
    rows = sorted(row_deficits)
    columns = sorted(column_deficits)
    source = 0
    row_node = {value: index + 1 for index, value in enumerate(rows)}
    column_offset = 1 + len(rows)
    column_node = {
        value: column_offset + index for index, value in enumerate(columns)
    }
    sink = column_offset + len(columns)
    graph: list[list[_Edge]] = [[] for _ in range(sink + 1)]

    required = sum(row_deficits.values())
    if required != sum(column_deficits.values()):
        raise ValueError("row and column margin deficits do not have equal totals")

    # Primary cost maximises the selected source remainders.  The binary
    # secondary term makes the selected-key bit vector lexicographically
    # maximal, which is exactly the sorted-cell-key tie break.  One unit of
    # primary cost outweighs the full possible secondary range.
    eligible = [key for key in keys if counts[key] % divisor]
    lexicographic_unit = 1 << len(eligible)
    primary_weight = required * lexicographic_unit + 1
    rank = {key: index for index, key in enumerate(eligible)}

    for row in rows:
        _add_edge(graph, source, row_node[row], row_deficits[row], 0)
    edge_refs: list[tuple[int, int, Cell]] = []
    for key in eligible:
        remainder = counts[key] % divisor
        preference = 1 << (len(eligible) - rank[key] - 1)
        secondary_cost = lexicographic_unit - preference
        node = row_node[row_of(key)]
        index = _add_edge(
            graph,
            node,
            column_node[column_of(key)],
            1,
            (divisor - remainder) * primary_weight + secondary_cost,
        )
        edge_refs.append((node, index, key))
    for column in columns:
        _add_edge(
            graph,
            column_node[column],
            sink,
            column_deficits[column],
            0,
        )

    # Successive shortest augmenting paths.  Initial forward costs are
    # non-negative; node potentials keep every residual reduced cost
    # non-negative after reverse edges appear, allowing deterministic Dijkstra.
    potentials = [0] * len(graph)
    flow = 0
    while flow < required:
        distances: list[int | None] = [None] * len(graph)
        previous: list[tuple[int, int] | None] = [None] * len(graph)
        distances[source] = 0
        queue = [(0, source)]
        while queue:
            distance, node = heapq.heappop(queue)
            if distance != distances[node]:
                continue
            for edge_index, edge in enumerate(graph[node]):
                if edge.capacity <= 0:
                    continue
                candidate = (
                    distance + edge.cost
                    + potentials[node] - potentials[edge.to]
                )
                current = distances[edge.to]
                if current is None or candidate < current:
                    distances[edge.to] = candidate
                    previous[edge.to] = (node, edge_index)
                    heapq.heappush(queue, (candidate, edge.to))
        if previous[sink] is None:
            raise ValueError(
                "rounded margins cannot be satisfied by the fractional cells"
            )
        for node, distance in enumerate(distances):
            if distance is not None:
                potentials[node] += distance

        node = sink
        while node != source:
            step = previous[node]
            if step is None:
                raise AssertionError("augmenting path ended before its source")
            prior, edge_index = step
            edge = graph[prior][edge_index]
            edge.capacity -= 1
            graph[node][edge.reverse].capacity += 1
            node = prior
        flow += 1

    return {
        key for node, edge_index, key in edge_refs
        if graph[node][edge_index].capacity == 0
    }


def scale_with_margins(
    counts: Mapping[Cell, int],
    divisor: int,
    *,
    row_of: Callable[[Cell], RowMargin],
    column_of: Callable[[Cell], ColumnMargin],
) -> ScaledCounts:
    """Scale cells while preserving apportioned row and column margins exactly."""
    keys = _validate_counts(counts, divisor)
    source_total = sum(counts.values())
    target_total = rounded_total(source_total, divisor)

    row_source: dict[RowMargin, int] = {}
    column_source: dict[ColumnMargin, int] = {}
    for key in keys:
        row = row_of(key)
        column = column_of(key)
        row_source[row] = row_source.get(row, 0) + counts[key]
        column_source[column] = column_source.get(column, 0) + counts[key]

    row_targets = largest_remainder(
        row_source, divisor, target=target_total
    )
    column_targets = largest_remainder(
        column_source, divisor, target=target_total
    )
    scaled = {key: counts[key] // divisor for key in keys}

    row_floors = {row: 0 for row in row_source}
    column_floors = {column: 0 for column in column_source}
    for key, value in scaled.items():
        row_floors[row_of(key)] += value
        column_floors[column_of(key)] += value
    row_deficits = {
        row: row_targets[row] - row_floors[row] for row in row_targets
    }
    column_deficits = {
        column: column_targets[column] - column_floors[column]
        for column in column_targets
    }

    increments = _minimum_cost_cell_increments(
        keys,
        counts,
        divisor,
        row_of,
        column_of,
        row_deficits,
        column_deficits,
    )
    for key in increments:
        scaled[key] += 1

    actual_rows = {row: 0 for row in row_targets}
    actual_columns = {column: 0 for column in column_targets}
    for key, value in scaled.items():
        actual_rows[row_of(key)] += value
        actual_columns[column_of(key)] += value
    if sum(scaled.values()) != target_total:
        raise AssertionError("constrained scaling did not preserve the total")
    if actual_rows != row_targets:
        raise AssertionError("constrained scaling did not preserve row margins")
    if actual_columns != column_targets:
        raise AssertionError("constrained scaling did not preserve column margins")

    return ScaledCounts(
        counts=scaled,
        total=target_total,
        row_margins=row_targets,
        column_margins=column_targets,
        errors={key: scaled[key] * divisor - counts[key] for key in keys},
        divisor=divisor,
    )
