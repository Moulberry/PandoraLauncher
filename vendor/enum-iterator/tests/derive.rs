// Copyright (c) 2018-2022 Stephane Raux. Distributed under the 0BSD license.

use enum_iterator::{all, cardinality, reverse_all, Sequence};
use std::{convert::Infallible, iter::once};

#[derive(Clone, Copy, Debug, PartialEq, Sequence)]
enum Direction {
    North,
    South,
    West,
    East,
}

#[derive(Clone, Debug, PartialEq, Sequence)]
enum Either<L, R> {
    Left(L),
    Right(R),
}

#[test]
fn all_values_of_generic_type_are_yielded() {
    assert_eq!(cardinality::<Either<bool, Direction>>(), 6);
    assert_eq!(
        all::<Either<bool, Direction>>().collect::<Vec<_>>(),
        [
            Either::Left(false),
            Either::Left(true),
            Either::Right(Direction::North),
            Either::Right(Direction::South),
            Either::Right(Direction::West),
            Either::Right(Direction::East),
        ]
    );
}

#[derive(Clone, Debug, PartialEq, Sequence)]
struct Foo<T: Copy> {
    x: T,
}

#[test]
fn all_values_of_generic_type_with_trait_bound_are_yielded() {
    assert_eq!(cardinality::<Foo<bool>>(), 2);
    assert_eq!(
        all::<Foo<bool>>().collect::<Vec<_>>(),
        [Foo { x: false }, Foo { x: true }],
    );
}

#[test]
fn all_values_of_enum_type_with_empty_variant_are_yielded() {
    assert_eq!(cardinality::<Either<Infallible, bool>>(), 2);
    assert_eq!(
        all::<Either<Infallible, bool>>().collect::<Vec<_>>(),
        [Either::Right(false), Either::Right(true)],
    );
}

#[derive(Debug, PartialEq, Sequence)]
struct Impossible {
    a: bool,
    b: Infallible,
}

#[test]
fn all_values_of_impossible_are_yielded() {
    assert_eq!(cardinality::<Impossible>(), 0);
    assert!(all::<Impossible>().next().is_none());
}

#[derive(Debug, PartialEq, Sequence)]
enum Move {
    Stay,
    Basic(Direction),
    Fancy(FancyMove),
    Jump {
        direction: Direction,
        somersault: bool,
    },
    Swim(Direction, SwimmingStyle),
}

#[derive(Debug, PartialEq, Sequence)]
struct FancyMove {
    direction: Direction,
    fast: bool,
}

#[derive(Debug, PartialEq, Sequence)]
enum SwimmingStyle {
    Breaststroke,
    FrontCrawl,
}

fn all_moves() -> Vec<Move> {
    let directions = [
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ];
    once(Move::Stay)
        .chain(directions.into_iter().map(Move::Basic))
        .chain(
            directions
                .into_iter()
                .flat_map(|d| [(d, false), (d, true)])
                .map(|(direction, fast)| FancyMove { direction, fast })
                .map(Move::Fancy),
        )
        .chain(
            directions
                .into_iter()
                .flat_map(|d| [(d, false), (d, true)])
                .map(|(direction, somersault)| Move::Jump {
                    direction,
                    somersault,
                }),
        )
        .chain(
            directions
                .into_iter()
                .flat_map(|d| {
                    [
                        (d, SwimmingStyle::Breaststroke),
                        (d, SwimmingStyle::FrontCrawl),
                    ]
                })
                .map(|(direction, style)| Move::Swim(direction, style)),
        )
        .collect()
}

#[test]
fn all_works() {
    assert_eq!(all::<Move>().collect::<Vec<_>>(), all_moves());
}

#[test]
fn reverse_all_works() {
    let expected = {
        let mut moves = all_moves();
        moves.reverse();
        moves
    };
    assert_eq!(reverse_all::<Move>().collect::<Vec<_>>(), expected);
}

#[derive(Debug, PartialEq, Sequence)]
enum Empty {}

#[test]
fn empty_cadinality_is_zero() {
    assert_eq!(cardinality::<Empty>(), 0);
}

#[test]
fn all_values_of_empty_are_yielded() {
    assert_eq!(all::<Empty>().collect::<Vec<_>>(), Vec::new());
}

#[test]
fn all_values_of_empty_are_yielded_in_reverse() {
    assert_eq!(reverse_all::<Empty>().collect::<Vec<_>>(), Vec::new());
}

#[derive(Debug, PartialEq, Sequence)]
struct Unit;

#[test]
fn unit_cardinality_is_one() {
    assert_eq!(cardinality::<Unit>(), 1);
}

#[test]
fn all_values_of_unit_are_yielded() {
    assert_eq!(all::<Unit>().collect::<Vec<_>>(), vec![Unit]);
}

#[test]
fn all_values_of_unit_are_yielded_in_reverse() {
    assert_eq!(reverse_all::<Unit>().collect::<Vec<_>>(), vec![Unit]);
}
