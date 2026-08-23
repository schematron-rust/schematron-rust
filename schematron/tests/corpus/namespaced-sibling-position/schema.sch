<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <ns prefix="p" uri="http://example.com/p"/>
  <pattern>
    <!-- Four element children of root, alternating between two namespaces
         but sharing the local name `a`. The reported location has to count
         a node's position among the siblings that its own name test would
         select — which means among siblings of the same local name *and*
         namespace, not local name alone. -->
    <rule context="root/*">
      <report test="true()">on <value-of select="name()"/> id <value-of select="@id"/></report>
    </rule>
  </pattern>
</schema>
