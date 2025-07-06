# A proposal for a new trust model on the Web

The current security model of the Web sits mostly on what is known as the Same Origin Policy (SOP): this ties cross-page capabilities to a domain name, delegating trust to the CA infrastructure.

This means that the mechanisms defining the trust relationship from a page to another are shared by the service providers (who can decide which domains they use) and the User Agent (UA) in charge of implementing proper isolation according to the SOP. Of course this has been abused by service providers who ended up using the same domain to share data among different apps by scoping them with url paths.

On the other hand, end users have no say on how two or more pages could trust each other. This is a very top down, rigid model. Instead, what about giving users ways to create their own "trust zones" that can include content from different sources?

A trust zone establishes its own origin, so trusted urls are of the form:
trusted://<zoneID>.<userID>/<zone specific path>

User IDs are used to fetch a zone description resource that lists the set of valid zones for that user and describes how to interpret the path component of the trusted url. A user ID can be:

- a DID for that user. The zone description resource url needs to be set in the DID document.
- a web domain controlled by the user. The zone description resource will then be fetched at the `.well-known/trusted-zones.json` relative url.

TODO: integrity checks of the zone resource.

# Trusted url example:

Let's consider that the resource at https://webbeef.org/.well-known/trusted-zones.json is:

```json
{
  "zones": [
    {
      "name": "demo",
      "mappings": [
        {
          "path": "port8000",
          "source": "https://localhost:8000/"
        },
        {
          "path": "port8080",
          "source": "https://localhost:8080/"
        }
      ]
    }
  ]
}
```

That will make these 2 urls same-origin, and would let them share eg. BroadcastChannel messages.

trusted://demo.webbeef.org/port8000/index.html
trusted://demo.webbeef.org/port8080/messages/send.html


