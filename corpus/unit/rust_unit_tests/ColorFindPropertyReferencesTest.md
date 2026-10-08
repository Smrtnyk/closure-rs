## ColorFindPropertyReferencesTest (post-call assertions, gate 0.2 (a))

Test class `com.google.javascript.jscomp.disambiguate.ColorFindPropertyReferencesTest` (reference commit bb8c8e7). Each test runs ColorFindPropertyReferences over a colorized AST (enableTypeCheck, replaceTypesWithColors) inside the hooked `test(...)` call, then asserts which use sites each PropertyClustering collected, keyed by the Color of a labelled statement (`assertThatUsesOf`), or which Colors the ColorGraphNodeFactory was asked for. Those checks link use-site Nodes to Color/ColorGraphNode objects by object identity, which the post-call dump records only as class-name refs. Port each as a Rust unit test next to the property-reference finder: build the colorized AST, run the finder with a stub color-graph-node factory, map each use site to its labelled color and token, and assert the facts below (including the @After checks: every use site's string equals its property name; the original-name cluster equals the color nodes of the expected labels).

- ColorFindPropertyReferencesTest#getProp_isFound: propIndex keySet == {a}; uses (label:token) a -> FOO:GETPROP
- ColorFindPropertyReferencesTest#optChainGetProp_isFound: propIndex keySet == {a}; uses (label:token) a -> FOO:OPTCHAIN_GETPROP
- ColorFindPropertyReferencesTest#objectLitProp_isFound: propIndex keySet == {a,b,c,d}; uses (label:token) a -> FOO:STRING_KEY, b -> FOO:MEMBER_FUNCTION_DEF, c -> FOO:GETTER_DEF, d -> FOO:SETTER_DEF
- ColorFindPropertyReferencesTest#objectPatternProp_isFound: propIndex keySet == {a}; uses (label:token) a -> FOO:STRING_KEY
- ColorFindPropertyReferencesTest#classMemberProp_onPrototype_isFound: propIndex keySet == {b,c,d,prototype}; uses (label:token) b/c/d -> FOO_PROTOTYPE:MEMBER_FUNCTION_DEF/GETTER_DEF/SETTER_DEF
- ColorFindPropertyReferencesTest#classMemberProp_onInstance_isFound: propIndex keySet == {a,constructor}; uses (label:token) a -> FOO:GETPROP
- ColorFindPropertyReferencesTest#classMemberField_onInstance_isFound: propIndex keySet == {a,b}; uses (label:token) a, b -> FOO:MEMBER_FIELD_DEF
- ColorFindPropertyReferencesTest#classMemberField_onCtor_isFound: propIndex keySet == {a,b}; uses (label:token) a, b -> TYPEOF_FOO:MEMBER_FIELD_DEF
- ColorFindPropertyReferencesTest#classMemberProp_onCtor_isFound: propIndex keySet == {b,c,d}; uses (label:token) b/c/d -> TYPEOF_FOO:MEMBER_FUNCTION_DEF/GETTER_DEF/SETTER_DEF
- ColorFindPropertyReferencesTest#objectLitProp_inObjectDefineProperties_isFound: propIndex keySet == {a,b,c,d,defineProperties,prototype}; uses (label:token) a/b/c/d -> FOO_PROTOTYPE:STRING_KEY/MEMBER_FUNCTION_DEF/GETTER_DEF/SETTER_DEF
- ColorFindPropertyReferencesTest#stringLiteralProp_viaPropDefinerFunction_isFound: uses (label:token) a -> FOO_PROTOTYPE:STRINGLIT (propertyReflectorNames {reflect})
- ColorFindPropertyReferencesTest#propDefinerFunction_noObject_associatesWithJavaNull: uses (label:token) a -> (no label: the use site maps to a color no labelled statement has):STRINGLIT
- ColorFindPropertyReferencesTest#externProps_areClusteredTogether: uses (label:token) a -> FOO, BAR, QUX:GETPROP; @After: the original-name cluster of a is exactly the color nodes of FOO, BAR, TUM
- ColorFindPropertyReferencesTest#externProps_areClusteredTogether_evenIfSourceInformationIsMissingWithinExterns: uses (label:token) as externProps_areClusteredTogether with extern source files cleared
- ColorFindPropertyReferencesTest#enumsAreClusteredWithExterns: uses (label:token) a -> FOO:GETPROP, BAR:STRING_KEY, QUX:GETPROP; @After: original-name cluster of a is the color nodes of FOO, BAR
- ColorFindPropertyReferencesTest#propertylessConstructorsAreRecordedInTypeFlattener: the colors the property finder asked the ColorGraphNodeFactory for are exactly the colors of TYPEOF_FOO, TYPEOF_BAR, TYPEOF_QUZ, TYPEOF_BAZ (constructors without properties are still recorded)
