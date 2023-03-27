//
// Copyright (C) 2023 Ariel Abreu
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

use std::mem::MaybeUninit;

pub(crate) trait CollectIntoArray<const LEN: usize>: Iterator {
	fn collect_into_array(&mut self) -> Option<[Self::Item; LEN]>;
}

impl<const LEN: usize, T: Iterator> CollectIntoArray<LEN> for T {
	fn collect_into_array(&mut self) -> Option<[Self::Item; LEN]> {
		// SAFETY: this is safe because `assume_init` is saying that the *array* is initialized, not its elements
		//         (and an array of MaybeUninit requires no initialization)
		let mut arr: [MaybeUninit<Self::Item>; LEN] =
			unsafe { MaybeUninit::uninit().assume_init() };

		for i in 0..LEN {
			match self.next() {
				Some(item) => arr[i].write(item),
				None => {
					for j in 0..i {
						// SAFETY: we already initialized the previous elements, so we can safely drop them
						unsafe { arr[j].assume_init_drop() }
					}

					return None;
				},
			};
		}

		// can't use this because it's currently broken for const generics:
		//     unsafe { std::mem::transmute::<_, [Self::Item; LEN]>(arr) }
		// instead, we map it to `assume_init` values. this *should* be optimized away
		// SAFETY: this is safe because we've already initialized the elements in the array
		Some(arr.map(|item| unsafe { item.assume_init() }))
	}
}
