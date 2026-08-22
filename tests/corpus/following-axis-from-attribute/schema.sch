<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="e">
      <!-- The `following` axis taken from an *attribute* node.
           XPath 1.0 orders an element's attributes after the element and
           before its children, and defines `following` as everything after
           the context node in document order bar the context node's own
           descendants. An attribute has no descendants, so nothing is
           excluded, and the element's children are therefore *following*
           its attributes. -->
      <report test="count(@x/following::a) = 3">from the attribute: the two inside e, plus the one after</report>
      <report test="count(following::a) = 1">from the element: only the one after</report>

      <!-- The other attribute-rooted steps, which every implementation
           agrees on, so that this case pins the contrast rather than one
           lone number. -->
      <report test="count(@x/preceding::a) = 1">preceding from the attribute: only the one before</report>
      <report test="name(@x/parent::*) = 'e'">the parent of an attribute is its element</report>
    </rule>
  </pattern>
</schema>
