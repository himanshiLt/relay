/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 * 
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<270401a0ef815400ae3917618a084a1a>>
 * @flow
 * @lightSyntaxTransform
 * @nogrep
 */

/* eslint-disable */

'use strict';

/*::
import type { ConcreteRequest, Query } from 'relay-runtime';
type usePreloadedQueryProvidedVariablesTest_badFragment$fragmentType = any;
export type usePreloadedQueryProvidedVariablesTest_badQuery$variables = {|
  id: string,
|};
export type usePreloadedQueryProvidedVariablesTest_badQuery$data = {|
  +node: ?{|
    +$fragmentSpreads: usePreloadedQueryProvidedVariablesTest_badFragment$fragmentType,
  |},
|};
export type usePreloadedQueryProvidedVariablesTest_badQuery = {|
  variables: usePreloadedQueryProvidedVariablesTest_badQuery$variables,
  response: usePreloadedQueryProvidedVariablesTest_badQuery$data,
|};
type ProvidedVariablesType = {|
  +__relay_internal__pv__RelayProvider_impure: {|
    +get: () => number,
  |},
|};
*/

var providedVariablesDefinition/*: ProvidedVariablesType*/ = {
  "__relay_internal__pv__RelayProvider_impure": require('./../RelayProvider_impure')
};

var node/*: ConcreteRequest*/ = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "id"
},
v1 = [
  {
    "kind": "Variable",
    "name": "id",
    "variableName": "id"
  }
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*: any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "usePreloadedQueryProvidedVariablesTest_badQuery",
    "selections": [
      {
        "alias": null,
        "args": (v1/*: any*/),
        "concreteType": null,
        "kind": "LinkedField",
        "name": "node",
        "plural": false,
        "selections": [
          {
            "args": null,
            "kind": "FragmentSpread",
            "name": "usePreloadedQueryProvidedVariablesTest_badFragment"
          }
        ],
        "storageKey": null
      }
    ],
    "type": "Query",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v0/*: any*/),
      {
        "defaultValue": null,
        "kind": "LocalArgument",
        "name": "__relay_internal__pv__RelayProvider_impure"
      }
    ],
    "kind": "Operation",
    "name": "usePreloadedQueryProvidedVariablesTest_badQuery",
    "selections": [
      {
        "alias": null,
        "args": (v1/*: any*/),
        "concreteType": null,
        "kind": "LinkedField",
        "name": "node",
        "plural": false,
        "selections": [
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "__typename",
            "storageKey": null
          },
          {
            "kind": "InlineFragment",
            "selections": [
              {
                "alias": null,
                "args": [
                  {
                    "kind": "Variable",
                    "name": "scale",
                    "variableName": "__relay_internal__pv__RelayProvider_impure"
                  }
                ],
                "concreteType": "Image",
                "kind": "LinkedField",
                "name": "profile_picture",
                "plural": false,
                "selections": [
                  {
                    "alias": null,
                    "args": null,
                    "kind": "ScalarField",
                    "name": "uri",
                    "storageKey": null
                  }
                ],
                "storageKey": null
              }
            ],
            "type": "User",
            "abstractKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "id",
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "6246b1bea212b475b9777fc29e6eb1d4",
    "id": null,
    "metadata": {},
    "name": "usePreloadedQueryProvidedVariablesTest_badQuery",
    "operationKind": "query",
    "text": "query usePreloadedQueryProvidedVariablesTest_badQuery(\n  $id: ID!\n  $__relay_internal__pv__RelayProvider_impure: Float!\n) {\n  node(id: $id) {\n    __typename\n    ...usePreloadedQueryProvidedVariablesTest_badFragment\n    id\n  }\n}\n\nfragment usePreloadedQueryProvidedVariablesTest_badFragment on User {\n  profile_picture(scale: $__relay_internal__pv__RelayProvider_impure) {\n    uri\n  }\n}\n",
    "providedVariables": providedVariablesDefinition
  }
};
})();

if (__DEV__) {
  (node/*: any*/).hash = "38101eec78c6bee2608fbe821589dd15";
}

module.exports = ((node/*: any*/)/*: Query<
  usePreloadedQueryProvidedVariablesTest_badQuery$variables,
  usePreloadedQueryProvidedVariablesTest_badQuery$data,
>*/);
